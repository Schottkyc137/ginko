use crate::dts::analysis::{Analyzer, CyclicDependencyEntry, PushIntoDiagnostics};
use crate::dts::ast::file::{FileItemKind, HeaderKind};
use crate::dts::ast::node::Node;
use crate::dts::ast::{file as ast, Cast};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::Eval;
use crate::dts::model::{NodeName, Path};
use crate::dts::{model, ErrorCode, FileType};
use itertools::Itertools;
use rowan::{TextRange, WalkEvent};
use std::collections::HashMap;
use std::path::PathBuf;

use super::project::ProjectState;

#[derive(Debug, Clone)]
pub struct LabelLocation {
    ast_path: Path,
    file: PathBuf,
    range: TextRange,
}

impl Analyzer {
    pub fn analyze_file(
        &self,
        project: &ProjectState,
        path: PathBuf,
        file_type: FileType,
        file: &ast::File,
        mut include_path: Vec<CyclicDependencyEntry>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> (model::File, LabelMap, FileType) {
        let mut dts_header_seen = false;
        let mut is_plugin = false;
        let mut reserved_memory = Vec::new();
        let mut root = model::Node::default();
        let mut labels = LabelMap::new();
        file.extract_labels(path, &mut labels, diagnostics);
        for child in file.children() {
            match child {
                FileItemKind::Header(header) => match header.kind() {
                    HeaderKind::DtsV1 => dts_header_seen = true,
                    HeaderKind::Plugin => is_plugin = true,
                },
                FileItemKind::Include(include) => {
                    if let Some(path) = include.target().map(PathBuf::from) {
                        let location = include.target_tok().unwrap().text_range();
                        let entry = CyclicDependencyEntry::new(path.clone(), location);
                        if include_path.contains(&entry) {
                            let diag_text = include_path
                                .clone()
                                .into_iter()
                                .map(|entry| entry.path().display().to_string())
                                .chain(std::iter::once(entry.path().display().to_string()))
                                .join(" -> ");
                            let diag = Diagnostic::new(
                                entry.location(),
                                ErrorCode::CyclicDependencyError,
                                format!("Cyclic dependency: {diag_text}"),
                            );
                            diagnostics.push(diag);
                            continue;
                        } else {
                            println!("Borrowing {entry:?}");
                            include_path.push(entry);
                            if let Some(cell) = project.get(&path) {
                                let mut analysis_diagnostics = Vec::new();
                                let (file, labels, kind) = {
                                    let path = cell.borrow().path().unwrap().clone();
                                    self.analyze_file(
                                        project,
                                        path,
                                        FileType::DtSourceInclude,
                                        cell.borrow().ast(),
                                        include_path.clone(),
                                        &mut analysis_diagnostics,
                                    )
                                };
                                let mut borrow = cell.borrow_mut();
                                borrow.set_analysis_result(file, labels, analysis_diagnostics);
                                borrow.set_kind(kind);
                            }
                        }
                    }
                }
                FileItemKind::ReserveMemory(reserved) => {
                    if let Some(mem) = self.analyze_memreserve(&reserved).or_push_into(diagnostics)
                    {
                        reserved_memory.push(mem)
                    }
                }
                FileItemKind::Node(node) => {
                    if let Some((name, body)) = self
                        .analyze_node(&node, diagnostics)
                        .or_push_into(diagnostics)
                    {
                        // TODO: referenced nodes
                        // TODO: duplicates
                        if name.is_root() {
                            root.merge(body)
                        } else {
                            diagnostics.push(Diagnostic::new(
                                node.range(),
                                ErrorCode::IllegalStart,
                                "Non root-node in root position",
                            ))
                        }
                    }
                }
            }
        }
        if file_type == FileType::DtSource && !dts_header_seen {
            diagnostics.push(Diagnostic::new(
                TextRange::default(),
                ErrorCode::NonDtsV1,
                "Missing /dts-v1/ header",
            ))
        }
        let out_file_type = if is_plugin {
            FileType::DtSourceOverlay
        } else {
            file_type
        };
        (
            model::File::new(root, reserved_memory),
            labels,
            out_file_type,
        )
    }

    pub fn analyze_memreserve(
        &self,
        reserve: &ast::ReserveMemory,
    ) -> Result<model::ReservedMemory, Diagnostic> {
        let address: u64 = reserve.address().eval()?;
        let length: u64 = reserve.length().eval()?;
        Ok(model::ReservedMemory { address, length })
    }
}

pub type LabelMap = HashMap<String, LabelLocation>;

impl ast::File {
    pub fn extract_labels(
        &self,
        file: PathBuf,
        labels: &mut LabelMap,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut path = Path::new();
        for event in self.walk().filter_map(|event| match event {
            WalkEvent::Enter(node) => Node::cast(node).map(WalkEvent::Enter),
            WalkEvent::Leave(node) => Node::cast(node).map(WalkEvent::Leave),
        }) {
            match event {
                WalkEvent::Enter(node) => {
                    match node.name().eval() {
                        Ok(name) => path.push(name),
                        Err(err) => {
                            diagnostics.push(err.into());
                            path.push(NodeName::simple(node.name().text()))
                        }
                    }

                    if let Some(label) = node.label() {
                        let location = LabelLocation {
                            ast_path: path.clone(),
                            file: file.clone(),
                            range: label.range(),
                        };
                        // TODO: duplicates
                        if let Some(previous) = labels.insert(label.to_string(), location) {
                            diagnostics.push(
                                Diagnostic::new(
                                    label.range(),
                                    ErrorCode::DuplicateLabel,
                                    "Duplicate label",
                                )
                                .with_related(
                                    previous.file,
                                    previous.range,
                                    "Previously defined here",
                                ),
                            )
                        }
                    }
                }
                WalkEvent::Leave(_) => {
                    path.pop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::dts::analysis::project::ProjectState;
    use crate::dts::analysis::{Analyzer, NoErrorAnalysis, WithDiagnosticAnalysis};
    use crate::dts::ast::file::File;
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::model::{Node, NodeBuilder, ReservedMemory};
    use crate::dts::{model, FileType};

    use super::LabelMap;

    impl WithDiagnosticAnalysis<(model::File, LabelMap, FileType)> for File {
        fn analyze_with_diagnostics(
            &self,
        ) -> (Option<(model::File, LabelMap, FileType)>, Vec<Diagnostic>) {
            let analyzer = Analyzer::new();
            let project = ProjectState::default();
            let mut diagnostics = Vec::new();
            let value = analyzer.analyze_file(
                &project,
                PathBuf::default(),
                crate::dts::FileType::DtSource,
                self,
                vec![],
                &mut diagnostics,
            );
            (Some(value), diagnostics)
        }
    }

    #[test]
    fn empty_file() {
        let (file, ..) = "\
/dts-v1/;

/ {};
        "
        .parse::<File>()
        .unwrap()
        .analyze_no_errors();
        assert_eq!(file.root(), &Node::default())
    }

    #[test]
    fn file_with_memreserve() {
        let (file, ..) = "\
/dts-v1/;

/memreserve/ 0x2000 0x4000;
/memreserve/ 0xAF3000 0x4000;

/ {};
        "
        .parse::<File>()
        .unwrap()
        .analyze_no_errors();
        assert_eq!(file.root(), &Node::default());
        assert_eq!(
            file.reserved_memory(),
            &[
                ReservedMemory {
                    address: 0x2000,
                    length: 0x4000
                },
                ReservedMemory {
                    address: 0xAF3000,
                    length: 0x4000
                }
            ]
        )
    }

    #[test]
    fn file_with_sub_nodes() {
        let (file, ..) = "\
/dts-v1/;

/ {
  node_a {
    prop_1 = <17>;
  };
};

/ {
  node_a {
    prop_2 = <42>;
  };

  node_b {
    node_c {
      prop_3 = [AB];
    };
  };
};
        "
        .parse::<File>()
        .unwrap()
        .analyze_no_errors();
        assert_eq!(
            file.root(),
            &NodeBuilder::new()
                .node(
                    "node_a",
                    NodeBuilder::new()
                        .property("prop_1", 17_u32)
                        .property("prop_2", 42_u32)
                )
                .node(
                    "node_b",
                    NodeBuilder::new()
                        .node("node_c", NodeBuilder::new().property("prop_3", [0xAB_u8]))
                )
                .build()
        );
    }
}
