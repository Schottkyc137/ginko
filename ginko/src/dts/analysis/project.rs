use crate::dts::analysis::{Analysis, AnalysisContext, CyclicDependencyEntry, PushIntoDiagnostics};
use crate::dts::ast::{Cast, FileItemKind, Include};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::lex::lex::lex;
use crate::dts::syntax::Parser;
use crate::dts::{ast, model, ErrorCode, FileType};
use itertools::Itertools;
use rowan::TextRange;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Project {
    state: ProjectState,
}

impl Project {
    pub fn add_file(&mut self, location: PathBuf, contents: &str) {
        let tokens = lex(contents);
        let parser = Parser::new(tokens.into_iter());
        let (node, syntax_diagnostics) = parser.parse(Parser::parse_file);
        let mut analysis_diagnostics = Vec::new();

        let node = ast::File::cast(node).unwrap();
        self.resolve_includes(&node, &mut analysis_diagnostics);
        let file_type = FileType::from(location.as_path());
        self.state.insert(
            location.clone(),
            ProjectFile {
                path: Some(location.clone()),
                source: contents.to_string(),
                kind: file_type,
                ast: node,
                syntax_diagnostics,
                model: None,
                analysis_diagnostics,
            },
        );
        self.analyze(&location);
    }

    pub fn analyze(&self, file: &PathBuf) {
        if let Some(cell) = self.state.files.get(file) {
            let mut analysis_diagnostics = Vec::new();
            let context = AnalysisContext {
                file_type: cell.borrow().kind,
                path: vec![CyclicDependencyEntry::new(
                    file.clone(),
                    TextRange::default(),
                )],
                ..Default::default()
            };
            let model = cell
                .borrow()
                .ast
                .analyze(&context, &self.state, &mut analysis_diagnostics)
                .or_push_into(&mut analysis_diagnostics);
            let mut mut_borrow = cell.borrow_mut();
            mut_borrow.set_analysis_result(model, analysis_diagnostics)
        }
    }

    fn resolve_includes(&mut self, file: &ast::File, diagnostics: &mut Vec<Diagnostic>) {
        let children = file
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
                match self.add_file_from_fs(path) {
                    Ok(_) => {}
                    Err(err) => diagnostics.push(Diagnostic::new(
                        location,
                        ErrorCode::IOError,
                        err.to_string(),
                    )),
                }
            }
        }
    }

    pub fn add_file_from_fs(&mut self, location: PathBuf) -> Result<(), io::Error> {
        let contents = std::fs::read_to_string(&location)?;
        self.add_file(location, &contents);
        Ok(())
    }

    pub fn add_include_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.state.include_paths.extend(paths);
    }

    pub fn project_files(&self) -> impl Iterator<Item = &RefCell<ProjectFile>> {
        self.state.files.values()
    }

    #[cfg(test)]
    pub fn add_raw_file(&mut self, file: ProjectFile) {
        self.state
            .files
            .insert(file.path.clone().unwrap(), RefCell::new(file));
    }

    pub fn get_file(&self, path: &PathBuf) -> Option<&RefCell<ProjectFile>> {
        self.state.get(path)
    }

    pub fn get_file_mut(&mut self, path: &PathBuf) -> Option<&mut RefCell<ProjectFile>> {
        self.state.get_mut(&path)
    }
}

#[derive(Debug)]
pub struct ProjectFile {
    path: Option<PathBuf>,
    source: String,
    kind: FileType,
    ast: ast::File,
    syntax_diagnostics: Vec<Diagnostic>,
    model: Option<model::File>,
    analysis_diagnostics: Vec<Diagnostic>,
}

#[cfg(test)]
impl ProjectFile {
    pub fn inline_with_path(path: impl Into<PathBuf>, text: impl Into<String>) -> ProjectFile {
        let path = path.into();
        let typ = FileType::from(path.as_path());
        let content = text.into();
        let ast: ast::File = content.parse().unwrap();
        ProjectFile {
            path: Some(path),
            source: content,
            kind: typ,
            ast,
            syntax_diagnostics: vec![],
            model: None,
            analysis_diagnostics: vec![],
        }
    }
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

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    pub fn ast(&self) -> &ast::File {
        &self.ast
    }

    pub fn set_analysis_result(&mut self, file: Option<model::File>, diagnostics: Vec<Diagnostic>) {
        self.model = file;
        self.analysis_diagnostics = diagnostics;
    }

    pub fn set_kind(&mut self, kind: FileType) {
        self.kind = kind
    }

    pub fn add_analysis_diagnostic(&mut self, diag: Diagnostic) {
        self.analysis_diagnostics.push(diag)
    }
}

#[derive(Default, Debug)]
pub struct ProjectState {
    files: HashMap<PathBuf, RefCell<ProjectFile>>,
    include_paths: Vec<PathBuf>,
}

impl ProjectState {
    pub fn new() -> ProjectState {
        ProjectState::default()
    }

    pub fn get_or_insert(&mut self, path: PathBuf) -> Result<&RefCell<ProjectFile>, io::Error> {
        if !self.files.contains_key(&path) {
            let contents = std::fs::read_to_string(&path)?;
            let tokens = lex(&contents);
            let parser = Parser::new(tokens.into_iter());
            let (node, diagnostics) = parser.parse(Parser::parse_file);
            self.files.insert(
                path.clone(),
                RefCell::new(ProjectFile {
                    path: Some(path.clone()),
                    source: contents,
                    kind: FileType::from(path.as_path()),
                    ast: ast::File::cast(node).unwrap(),
                    syntax_diagnostics: diagnostics,
                    model: None,
                    analysis_diagnostics: Vec::new(),
                }),
            );
        }
        Ok(self.files.get(&path).unwrap())
    }

    pub fn analyze(&self, file: &PathBuf, context: AnalysisContext) {
        if let Some(cell) = self.files.get(file) {
            let mut analysis_diagnostics = Vec::new();
            let model = cell
                .borrow()
                .ast
                .analyze(&context, self, &mut analysis_diagnostics)
                .or_push_into(&mut analysis_diagnostics);
            let mut borrow = cell.borrow_mut();
            borrow.set_analysis_result(model, analysis_diagnostics);
            borrow.set_kind(context.file_type);
        }
    }
}

impl ProjectState {
    pub fn insert(&mut self, key: PathBuf, value: ProjectFile) {
        self.files.insert(key, RefCell::new(value));
    }

    pub fn get(&self, path: &PathBuf) -> Option<&RefCell<ProjectFile>> {
        self.files.get(path)
    }

    pub fn get_mut(&mut self, path: &PathBuf) -> Option<&mut RefCell<ProjectFile>> {
        self.files.get_mut(path)
    }
}

impl Include {
    pub fn resolve<'a>(
        &self,
        project: &'a mut ProjectState,
    ) -> Option<Result<&'a RefCell<ProjectFile>, io::Error>> {
        let target: PathBuf = self.target()?.into();
        Some(project.get_or_insert(target))
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::project::{Project, ProjectFile};
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::{ErrorCode, FileType};
    use itertools::Itertools;
    use rowan::{TextRange, TextSize};
    use std::path::PathBuf;

    #[test]
    fn multi_includes() {
        let mut project = Project::default();
        project.add_raw_file(ProjectFile::inline_with_path(
            "file2.dts",
            r#"
/include/ "file3.dts"
        "#,
        ));
        project.add_raw_file(ProjectFile::inline_with_path("file3.dts", r#""#));
        project.add_file(
            PathBuf::from("file1.dts"),
            r#"
/dts-v1/;

/include/ "file2.dts"
        "#,
        );
        let file = project
            .get_file(&PathBuf::from("file1.dts"))
            .unwrap()
            .borrow();
        assert!(file.syntax_diagnostics.is_empty());
        assert!(file.analysis_diagnostics.is_empty());
        assert_eq!(file.kind, FileType::DtSource);
        assert!(file.model.is_some());

        let file = project
            .get_file(&PathBuf::from("file2.dts"))
            .unwrap()
            .borrow();
        assert!(file.syntax_diagnostics.is_empty());
        assert!(file.analysis_diagnostics.is_empty());
        // even though file2 and file3 were declared as '.dts', their type should change to include
        assert_eq!(file.kind, FileType::DtSourceInclude);
        assert!(file.model.is_some());

        let file = project
            .get_file(&PathBuf::from("file3.dts"))
            .unwrap()
            .borrow();
        assert!(file.syntax_diagnostics.is_empty());
        assert!(file.analysis_diagnostics.is_empty());
        assert_eq!(file.kind, FileType::DtSourceInclude);
        assert!(file.model.is_some());
    }

    #[test]
    fn cyclic_includes() {
        let mut project = Project::default();
        project.add_raw_file(ProjectFile::inline_with_path(
            "file2.dts",
            r#"/include/ "file3.dts""#,
        ));
        project.add_raw_file(ProjectFile::inline_with_path(
            "file3.dts",
            r#"/include/ "file1.dts""#,
        ));
        project.add_file(
            PathBuf::from("file1.dts"),
            r#"
/dts-v1/;

/include/ "file2.dts"
        "#,
        );
        let file = project
            .get_file(&PathBuf::from("file3.dts"))
            .unwrap()
            .borrow();
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
