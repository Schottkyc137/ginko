use crate::dts::analysis::AnalysisContext;
use crate::dts::ast::{DtsFile, Reference};
use crate::dts::data::HasSource;
use crate::dts::diagnostics::DiagnosticKind;
use crate::dts::lexer::Lexer;
use crate::dts::reader::ByteReader;
use crate::dts::visitor::ItemAtCursor;
use crate::dts::{Diagnostic, FileType, HasSpan, Parser, Position, SeverityLevel, Span};
use std::collections::HashMap;
use std::fs;
use std::iter::empty;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default)]
pub struct ProjectFile {
    pub(crate) parser_diagnostics: Vec<Diagnostic>,
    pub(crate) analysis_diagnostics: Vec<Diagnostic>,
    pub(crate) file: Option<DtsFile>,
    pub(crate) context: Option<AnalysisContext>,
    pub(crate) _file_type: FileType,
}

impl ProjectFile {
    pub fn analysis_context(&self) -> Option<&AnalysisContext> {
        self.context.as_ref()
    }

    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.parser_diagnostics
            .iter()
            .chain(&self.analysis_diagnostics)
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics()
            .any(|diagnostic| diagnostic.default_severity() == SeverityLevel::Error)
    }
}

#[derive(Default)]
pub struct Project {
    files: HashMap<PathBuf, ProjectFile>,
}

impl Project {
    pub fn add_file(&mut self, path: PathBuf, text: String, file_type: FileType) {
        // First step: Parse file and all dependencies.
        // Dependencies are cached.
        self.parse_file(path.clone(), text, file_type);
        // Second step: Analyze
    }

    pub fn remove_file(&mut self, path: &PathBuf) {
        self.files.remove(path);
    }

    pub fn get_diagnostics(&self, path: &Path) -> Box<dyn Iterator<Item = &Diagnostic> + '_> {
        let Some(file) = self.files.get(path) else {
            return Box::new(empty());
        };
        Box::new(file.diagnostics())
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

    pub fn get_node_position(
        &self,
        path: &PathBuf,
        reference: &Reference,
    ) -> Option<(Span, Arc<Path>)> {
        let referenced = self.get_analysis(path)?.get_referenced(reference)?;
        Some((referenced.name.span(), referenced.name.source()))
    }

    pub fn get_root(&self, path: &PathBuf) -> Option<&DtsFile> {
        match self.files.get(path) {
            Some(ProjectFile {
                file: Some(file), ..
            }) => Some(file),
            _ => None,
        }
    }

    fn parse_file(&mut self, file_name: PathBuf, text: String, file_type: FileType) {
        let reader = ByteReader::from_string(text);
        let lexer = Lexer::new(reader, file_name.clone().into());
        let mut parser = Parser::new(lexer);
        match parser.file() {
            Ok(file) => {
                // insert dummy file to be defined so that no cyclic dependency can occur.
                self.files.insert(file_name.clone(), ProjectFile::default());
                for include in file
                    .elements
                    .iter()
                    .filter_map(|primary| primary.as_include())
                {
                    let path = include.path();
                    // Avoids duplicate insertion and cyclic dependencies
                    if self.files.contains_key(&path) {
                        continue;
                    }
                    match fs::read_to_string(&path) {
                        Ok(text) => {
                            let typ = FileType::from(path.as_path());
                            self.parse_file(path, text, typ);
                        }
                        Err(err) => parser.diagnostics.push(Diagnostic::from_token(
                            include.include_token.clone(),
                            DiagnosticKind::from(err),
                        )),
                    }
                }
                self.files.insert(
                    file_name,
                    ProjectFile {
                        parser_diagnostics: parser.diagnostics,
                        file: Some(file),
                        _file_type: file_type,
                        context: None,
                        analysis_diagnostics: vec![],
                    },
                );
            }
            Err(err) => {
                self.files.insert(
                    file_name,
                    ProjectFile {
                        parser_diagnostics: vec![err],
                        analysis_diagnostics: vec![],
                        file: None,
                        context: None,
                        _file_type: file_type,
                    },
                );
            }
        };
    }

    pub fn get_file(&self, path: &Path) -> Option<&ProjectFile> {
        self.files.get(path)
    }

    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.files.values().flat_map(|val| &val.parser_diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::diagnostics::DiagnosticKind;
    use crate::dts::lexer::TokenKind;
    use crate::dts::test::Code;
    use crate::dts::{Diagnostic, FileType, HasSpan, Project};
    use itertools::Itertools;
    use std::path::PathBuf;

    #[test]
    pub fn file_with_includes() {
        let mut project = Project::default();
        let code = "";
        let path: PathBuf = "path/to/file.dtsi".into();
        project.add_file(path.clone(), code.to_owned(), FileType::DtSourceInclude);

        let code2 = r#"
/dts-v1/;

/include/ "path/to/file.dtsi"
"#;
        let path2: PathBuf = "path/to/other/file.dts".into();
        project.add_file(path2.clone(), code2.to_owned(), FileType::DtSource);
        assert!(project.get_file(&path).is_some());
        assert!(project.get_file(&path2).is_some());
        assert_eq!(project.get_diagnostics(&path).next(), None);
        assert_eq!(project.get_diagnostics(&path2).next(), None);
        assert!(project.get_root(&path).is_some());
        assert!(project.get_analysis(&path).is_some());
        assert!(project.get_analysis(&path2).is_some());
    }

    #[test]
    pub fn error_in_included_file_add_include_before_dts() {
        let mut project = Project::default();
        let code = Code::new("/ {}"); // missing semicolon
        let path: PathBuf = "path/to/file.dtsi".into();
        project.add_file(
            path.clone(),
            code.code().to_owned(),
            FileType::DtSourceInclude,
        );
        let code2 = Code::new(
            r#"
/dts-v1/;

/include/ "path/to/file.dtsi"
"#,
        );
        let path2: PathBuf = "path/to/other/file.dts".into();
        project.add_file(path2.clone(), code2.code().to_owned(), FileType::DtSource);

        assert!(project.get_file(&path).is_some());
        assert!(project.get_file(&path2).is_some());
        assert_eq!(
            project.get_diagnostics(&path).cloned().collect_vec(),
            vec![Diagnostic::new(
                code.s1("}").end().as_span(),
                path.clone().into(),
                DiagnosticKind::Expected(vec![TokenKind::Semicolon])
            )]
        );
        assert_eq!(
            project.get_diagnostics(&path2).cloned().collect_vec(),
            vec![Diagnostic::new(
                code2.s1(r#"/include/ "path/to/file.dtsi""#).span(),
                path2.clone().into(),
                DiagnosticKind::ErrorsInInclude
            )]
        );
    }
}
