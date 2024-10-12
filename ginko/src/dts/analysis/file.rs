use crate::dts::analysis::{Analysis, AnalysisContext, PushIntoDiagnostics};
use crate::dts::ast::file as ast;
use crate::dts::ast::file::{FileItemKind, HeaderKind};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::Eval;
use crate::dts::{model, ErrorCode};
use rowan::TextRange;

impl Analysis<model::File> for ast::File {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<model::File, Diagnostic> {
        let mut dts_header_seen = false;
        let mut is_plugin = false;
        let mut reserved_memory = Vec::new();
        let mut root = model::Node::default();
        for child in self.children() {
            match child {
                FileItemKind::Header(header) => match header.kind() {
                    HeaderKind::DtsV1 => dts_header_seen = true,
                    HeaderKind::Plugin => is_plugin = true,
                },
                FileItemKind::Include(include) => {}
                FileItemKind::ReserveMemory(reserved) => {
                    if let Some(mem) = reserved
                        .analyze(context, diagnostics)
                        .or_push_into(diagnostics)
                    {
                        reserved_memory.push(mem)
                    }
                }
                FileItemKind::Node(node) => {
                    if let Some((name, body)) =
                        node.analyze(context, diagnostics).or_push_into(diagnostics)
                    {
                        // TODO: referenced nodes
                        // TODO: duplicates
                        if name == "/" {
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
        if !dts_header_seen {
            diagnostics.push(Diagnostic::new(
                TextRange::default(),
                ErrorCode::NonDtsV1,
                "Missing /dts-v1/ header",
            ))
        }
        Ok(model::File::new(root, reserved_memory))
    }
}

impl Analysis<model::ReservedMemory> for ast::ReserveMemory {
    fn analyze(
        &self,
        _: &AnalysisContext,
        _: &mut Vec<Diagnostic>,
    ) -> Result<model::ReservedMemory, Diagnostic> {
        let address: u64 = self.address().eval()?;
        let length: u64 = self.length().eval()?;
        Ok(model::ReservedMemory { address, length })
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::NoErrorAnalysis;
    use crate::dts::ast::file::File;
    use crate::dts::model::{Node, NodeBuilder, ReservedMemory};

    #[test]
    fn empty_file() {
        let file = "\
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
        let file = "\
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
        let file = "\
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
