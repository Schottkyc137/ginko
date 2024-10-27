use super::cyclic_dependency::CyclicDependencyEntry;
use super::Analyzer;
use crate::dts::analysis::file::LabelMap;
use crate::dts::ast::{FileItemKind, Include};
use crate::dts::ast2::Reference;
use crate::dts::diagnostics::Diagnostic;
use crate::dts::lex::lex;
use crate::dts::syntax::{Parser, SyntaxNode};
use crate::dts::{ast, model, ErrorCode, FileType, ItemAtCursor};
use itertools::Itertools;
use parking_lot::RwLock;
use rowan::{GreenNode, TextRange, TextSize};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Project {
    state: ProjectState,
}

impl Project {
    /// Add a file to the project and parse it.
    /// No analysis will be performed at this point.
    pub fn add_file(&mut self, location: PathBuf, contents: &str) {
        let tokens = lex(contents);
        let parser = Parser::new(tokens.into_iter());
        let (green, syntax_diagnostics) = parser.parse_to_green(Parser::parse_file);

        let file_type = FileType::from(location.as_path());
        self.state.insert(
            location.clone(),
            ProjectFile {
                path: location.clone(),
                source: contents.to_string(),
                kind: file_type,
                ast: green,
                syntax_diagnostics,
                model: None,
                labels: HashMap::default(),
                analysis_diagnostics: vec![],
            },
        );
    }

    /// Read the contents of the file at `location` and add it to the project.
    /// Similar to `add_file`, no analysis will be performed in this step.
    pub fn add_file_from_fs(&mut self, location: &PathBuf) -> Result<(), io::Error> {
        let contents = std::fs::read_to_string(location)?;
        self.add_file(location.clone(), &contents);
        Ok(())
    }

    /// Analyze the file at `file`.
    /// If the file can't be found, this is a noop.
    pub fn analyze(&mut self, file: &PathBuf) {
        self.resolve_includes(file);
        self.state.analyze(file);
    }

    /// Resolve includes from the file denoted by `path`
    fn resolve_includes(&mut self, path: &PathBuf) {
        let mut diagnostics = Vec::new();
        if let Some(cell) = self.state.files.get(path) {
            let children = cell
                .read()
                .ast()
                .children()
                .filter_map(|kind| match kind {
                    FileItemKind::Include(include) => Some(include),
                    _ => None,
                })
                .filter_map(|include| {
                    include.target().map(|target| {
                        (
                            PathBuf::from(target),
                            include.target_tok().unwrap().text_range(),
                        )
                    })
                })
                .collect_vec();

            for (path, location) in children {
                if !self.state.files.contains_key(&path) {
                    match self.add_file_from_fs(&path) {
                        Ok(_) => self.resolve_includes(&path),
                        Err(err) => diagnostics.push(Diagnostic::new(
                            location,
                            ErrorCode::IOError,
                            err.to_string(),
                        )),
                    }
                }
            }
        }
        if let Some(cell) = self.state.files.get(path) {
            cell.write().analysis_diagnostics.extend(diagnostics);
        }
    }

    pub fn add_include_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.state.include_paths.extend(paths);
    }

    pub fn project_files(&self) -> impl Iterator<Item = &RwLock<ProjectFile>> {
        self.state.files.values()
    }

    pub fn get_file(&self, path: &PathBuf) -> Option<&RwLock<ProjectFile>> {
        self.state.get(path)
    }

    pub fn get_file_mut(&mut self, path: &PathBuf) -> Option<&mut RwLock<ProjectFile>> {
        self.state.get_mut(path)
    }

    pub fn files(&self) -> impl Iterator<Item = &PathBuf> {
        self.state.files.keys()
    }

    pub fn set_include_paths(&mut self, includes: impl Iterator<Item = PathBuf>) {
        self.state.include_paths = includes.collect();
    }

    pub fn remove_file(&mut self, path: &PathBuf) {
        // TODO
    }

    pub fn get_node_position(
        &self,
        path: &PathBuf,
        reference: &Reference,
    ) -> Option<(TextRange, PathBuf)> {
        // TODO
        None
    }

    pub fn document_reference(&self, path: &PathBuf, reference: &Reference) -> Option<String> {
        // TODO
        None
    }
}

#[derive(Debug)]
pub struct ProjectFile {
    path: PathBuf,
    source: String,
    kind: FileType,
    ast: GreenNode,
    syntax_diagnostics: Vec<Diagnostic>,
    model: Option<model::File>,
    labels: LabelMap,
    analysis_diagnostics: Vec<Diagnostic>,
}

impl ProjectFile {
    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.syntax_diagnostics
            .iter()
            .chain(self.analysis_diagnostics.iter())
    }

    pub fn source(&self) -> &str {
        self.source.as_str()
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn ast(&self) -> ast::File {
        ast::File::cast_unchecked(SyntaxNode::new_root(self.ast.clone()))
    }

    pub fn set_analysis_result(
        &mut self,
        file: model::File,
        labels: LabelMap,
        diagnostics: Vec<Diagnostic>,
    ) {
        self.model = Some(file);
        self.labels = labels;
        self.analysis_diagnostics.extend(diagnostics);
    }

    pub fn set_kind(&mut self, kind: FileType) {
        self.kind = kind
    }

    pub fn add_analysis_diagnostic(&mut self, diag: Diagnostic) {
        self.analysis_diagnostics.push(diag)
    }

    pub fn item_at_cursor<'a>(&self, cursor: TextSize) -> Option<ItemAtCursor<'a>> {
        // TODO
        None
    }
}

#[derive(Default, Debug)]
pub struct ProjectState {
    files: HashMap<PathBuf, RwLock<ProjectFile>>,
    include_paths: Vec<PathBuf>,
}

impl ProjectState {
    pub fn new() -> ProjectState {
        ProjectState::default()
    }

    pub fn get_or_insert(&mut self, path: PathBuf) -> Result<&RwLock<ProjectFile>, io::Error> {
        if !self.files.contains_key(&path) {
            let contents = std::fs::read_to_string(&path)?;
            let tokens = lex(&contents);
            let parser = Parser::new(tokens.into_iter());
            let (node, diagnostics) = parser.parse_to_green(Parser::parse_file);
            self.files.insert(
                path.clone(),
                RwLock::new(ProjectFile {
                    path: path.clone(),
                    source: contents,
                    kind: FileType::from(path.as_path()),
                    ast: node,
                    syntax_diagnostics: diagnostics,
                    model: None,
                    labels: HashMap::default(),
                    analysis_diagnostics: Vec::new(),
                }),
            );
        }
        Ok(self.files.get(&path).unwrap())
    }

    pub fn analyze(&self, file: &PathBuf) {
        if let Some(cell) = self.files.get(file) {
            let mut analysis_diagnostics = Vec::new();
            let analyzer = Analyzer::new();
            let path = cell.read().path().clone();
            let file_type = FileType::from(path.as_path());
            let cde = CyclicDependencyEntry::new(path.clone(), TextRange::default());
            let (file, labels, kind) = analyzer.analyze_file(
                self,
                path,
                file_type,
                cell.read().ast(),
                vec![cde],
                &mut analysis_diagnostics,
            );
            let mut borrow = cell.write();
            borrow.set_analysis_result(file, labels, analysis_diagnostics);
            borrow.set_kind(kind);
        }
    }
}

impl ProjectState {
    pub fn insert(&mut self, key: PathBuf, value: ProjectFile) {
        self.files.insert(key, RwLock::new(value));
    }

    pub fn get(&self, path: &PathBuf) -> Option<&RwLock<ProjectFile>> {
        self.files.get(path)
    }

    pub fn get_mut(&mut self, path: &PathBuf) -> Option<&mut RwLock<ProjectFile>> {
        self.files.get_mut(path)
    }
}

impl Include {
    pub fn resolve<'a>(
        &self,
        project: &'a mut ProjectState,
    ) -> Option<Result<&'a RwLock<ProjectFile>, io::Error>> {
        let target: PathBuf = self.target()?.into();
        Some(project.get_or_insert(target))
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::project::Project;
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::{ErrorCode, FileType};
    use rowan::{TextRange, TextSize};
    use std::path::PathBuf;

    #[test]
    fn multi_includes() {
        let mut project = Project::default();
        project.add_file(
            PathBuf::from("file2.dts"),
            r#"
/include/ "file3.dts"
        "#,
        );
        project.add_file(PathBuf::from("file3.dts"), r#""#);
        project.add_file(
            PathBuf::from("file1.dts"),
            r#"
/dts-v1/;

/include/ "file2.dts"
        "#,
        );
        project.analyze(&PathBuf::from("file1.dts"));
        let file = project
            .get_file(&PathBuf::from("file1.dts"))
            .unwrap()
            .read();
        assert!(file.syntax_diagnostics.is_empty());
        assert!(file.analysis_diagnostics.is_empty());
        assert_eq!(file.kind, FileType::DtSource);
        assert!(file.model.is_some());

        let file = project
            .get_file(&PathBuf::from("file2.dts"))
            .unwrap()
            .read();
        assert!(file.syntax_diagnostics.is_empty());
        assert!(file.analysis_diagnostics.is_empty());
        // even though file2 and file3 were declared as '.dts', their type should change to include
        assert_eq!(file.kind, FileType::DtSourceInclude);
        assert!(file.model.is_some());

        let file = project
            .get_file(&PathBuf::from("file3.dts"))
            .unwrap()
            .read();
        assert!(file.syntax_diagnostics.is_empty());
        assert!(file.analysis_diagnostics.is_empty());
        assert_eq!(file.kind, FileType::DtSourceInclude);
        assert!(file.model.is_some());
    }

    #[test]
    fn cyclic_includes() {
        let mut project = Project::default();
        project.add_file(PathBuf::from("file2.dts"), r#"/include/ "file3.dts""#);
        project.add_file(PathBuf::from("file3.dts"), r#"/include/ "file1.dts""#);
        project.add_file(
            PathBuf::from("file1.dts"),
            r#"
/dts-v1/;

/include/ "file2.dts"
        "#,
        );
        project.analyze(&PathBuf::from("file1.dts"));
        let file = project
            .get_file(&PathBuf::from("file3.dts"))
            .unwrap()
            .read();
        assert_eq!(
            file.analysis_diagnostics,
            vec![Diagnostic::new(
                TextRange::new(TextSize::new(10), TextSize::new(21)),
                ErrorCode::CyclicDependencyError,
                "Cyclic dependency: file1.dts -> file2.dts -> file3.dts -> file1.dts".to_string()
            )]
        )
    }
}
