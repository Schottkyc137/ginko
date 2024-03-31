use crate::dts::analysis::AnalysisContext;
use crate::dts::ast::{DtsFile, Reference};
use crate::dts::importer::{CyclicDependencyChecker, CyclicDependencyError};
use crate::dts::lexer::Lexer;
use crate::dts::reader::ByteReader;
use crate::dts::visitor::ItemAtCursor;
use crate::dts::{Analysis, Diagnostic, FileType, HasSpan, Parser, Position, SeverityLevel, Span};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub trait FileManager {
    fn get_file(&self, path: &Path) -> Option<&ProjectFile>;

    fn add_file_with_parent(
        &mut self,
        path: PathBuf,
        parent: Option<PathBuf>,
        text: String,
        file_type: FileType,
    ) -> Result<&ProjectFile, CyclicDependencyError<PathBuf>>;
}

pub struct ProjectFile {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) file: Option<DtsFile>,
    pub(crate) context: Option<AnalysisContext>,
}

impl ProjectFile {
    pub fn analysis_context(&self) -> Option<&AnalysisContext> {
        self.context.as_ref()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.default_severity() == SeverityLevel::Error)
            .next()
            .is_some()
    }
}

#[derive(Default)]
pub struct Project {
    files: HashMap<PathBuf, ProjectFile>,
    importer: CyclicDependencyChecker<PathBuf>,
}

static NO_DIAGNOSTICS: Vec<Diagnostic> = Vec::new();

impl Project {
    pub fn add_file(&mut self, path: PathBuf, text: String, file_type: FileType) {
        let _ = self.add_file_with_parent(path, None, text, file_type);
    }

    pub fn remove_file(&mut self, path: &PathBuf) {
        self.files.remove(path);
    }

    pub fn get_diagnostics(&self, path: &Path) -> &[Diagnostic] {
        self.files
            .get(path)
            .map(|file| &file.diagnostics)
            .unwrap_or(&NO_DIAGNOSTICS)
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.files.keys().map(|key| key.as_path())
    }

    pub fn get_analysis(&self, path: &PathBuf) -> Option<&AnalysisContext> {
        match self.files.get(path) {
            None => None,
            Some(project) => project.context.as_ref(),
        }
    }

    pub fn find_at_pos<'a>(
        &'a self,
        path: &PathBuf,
        position: &Position,
    ) -> Option<ItemAtCursor<'a>> {
        let file = match self.files.get(path).and_then(|file| file.file.as_ref()) {
            None => return None,
            Some(file) => file,
        };
        file.item_at_cursor(position)
    }

    pub fn document_reference(&self, path: &PathBuf, reference: &Reference) -> Option<String> {
        let referenced = self.get_analysis(path)?.get_referenced(reference)?;
        Some(format!("Node {}", referenced.name.name.clone()))
    }

    pub fn get_node_position(&self, path: &PathBuf, reference: &Reference) -> Option<Span> {
        let referenced = self.get_analysis(path)?.get_referenced(reference)?;
        Some(referenced.name.span())
    }

    pub fn get_root(&self, path: &PathBuf) -> Option<&DtsFile> {
        match self.files.get(path) {
            Some(ProjectFile {
                file: Some(file), ..
            }) => Some(file),
            _ => None,
        }
    }

    pub(crate) fn analyze_text(
        &mut self,
        text: String,
        file_name: PathBuf,
        file_type: FileType,
    ) -> ProjectFile {
        let reader = ByteReader::from_string(text);
        let lexer = Lexer::new(reader, file_name.into());
        let mut parser = Parser::new(lexer);
        match parser.file() {
            Ok(file) => {
                let mut analysis = Analysis::new(file_type, self);
                analysis.analyze_file(&mut parser.diagnostics, &file);
                ProjectFile {
                    diagnostics: parser.diagnostics,
                    file: Some(file),
                    context: Some(analysis.into_context()),
                }
            }
            Err(err) => ProjectFile {
                diagnostics: vec![err],
                file: None,
                context: None,
            },
        }
    }

    #[cfg(test)]
    pub fn add_raw_file(&mut self, path: PathBuf, file: ProjectFile) {
        self.files.insert(path, file);
    }
}

impl FileManager for Project {
    fn get_file(&self, path: &Path) -> Option<&ProjectFile> {
        self.files.get(path)
    }

    fn add_file_with_parent(
        &mut self,
        path: PathBuf,
        parent: Option<PathBuf>,
        text: String,
        file_type: FileType,
    ) -> Result<&ProjectFile, CyclicDependencyError<PathBuf>> {
        if self.files.contains_key(&path) {
            return Ok(self.files.get(&path).unwrap());
        }
        if let Some(parent) = parent {
            self.importer.add(parent, &[path.clone()])?;
        } else {
            self.importer.add(path.clone(), &[])?;
        }
        let file = self.analyze_text(text, path.clone(), file_type);
        self.files.insert(path.clone(), file);
        Ok(self.files.get(&path).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::diagnostics::DiagnosticKind;
    use crate::dts::lexer::TokenKind;
    use crate::dts::project::FileManager;
    use crate::dts::test::Code;
    use crate::dts::{Diagnostic, FileType, HasSpan, Project};
    use std::path::PathBuf;

    #[test]
    pub fn file_with_includes() {
        let mut project = Project::default();
        let code = "";
        let path: PathBuf = "path/to/file.dtsi".into();
        project
            .add_file_with_parent(
                path.clone(),
                None,
                code.to_owned(),
                FileType::DtSourceInclude,
            )
            .expect("No cyclic dependencies expected");
        let code2 = r#"
/dts-v1/;

/include/ "path/to/file.dtsi"
"#;
        let path2: PathBuf = "path/to/other/file.dts".into();
        project
            .add_file_with_parent(path2.clone(), None, code2.to_owned(), FileType::DtSource)
            .expect("No cyclic dependencies expected");
        assert!(project.get_file(&path).is_some());
        assert!(project.get_file(&path2).is_some());
        assert!(project.get_diagnostics(&path).is_empty());
        assert!(project.get_diagnostics(&path2).is_empty());
        assert!(project.get_root(&path).is_some());
        assert!(project.get_analysis(&path).is_some());
        assert!(project.get_analysis(&path2).is_some());
    }

    #[test]
    pub fn error_in_included_file_add_include_before_dts() {
        let mut project = Project::default();
        let code = Code::new("/ {}"); // missing semicolon
        let path: PathBuf = "path/to/file.dtsi".into();
        project
            .add_file_with_parent(
                path.clone(),
                None,
                code.code().to_owned(),
                FileType::DtSourceInclude,
            )
            .expect("No cyclic dependencies expected");
        let code2 = Code::new(
            r#"
/dts-v1/;

/include/ "path/to/file.dtsi"
"#,
        );
        let path2: PathBuf = "path/to/other/file.dts".into();
        project
            .add_file_with_parent(
                path2.clone(),
                None,
                code2.code().to_owned(),
                FileType::DtSource,
            )
            .expect("No cyclic dependencies expected");
        assert!(project.get_file(&path).is_some());
        assert!(project.get_file(&path2).is_some());
        assert_eq!(
            project.get_diagnostics(&path),
            vec![Diagnostic::new(
                code.s1("}").end().as_span(),
                path.clone().into(),
                DiagnosticKind::Expected(vec![TokenKind::Semicolon])
            )]
        );
        assert_eq!(
            project.get_diagnostics(&path2),
            vec![Diagnostic::new(
                code2.s1(r#"/include/ "path/to/file.dtsi""#).span(),
                path2.clone().into(),
                DiagnosticKind::ErrorsInInclude
            )]
        );
    }
}
