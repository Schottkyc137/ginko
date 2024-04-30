use crate::dts::analysis::{Analysis, AnalysisContext};
use crate::dts::ast::{DtsFile, Reference};
use crate::dts::data::HasSource;
use crate::dts::diagnostics::DiagnosticKind;
use crate::dts::lexer::Lexer;
use crate::dts::reader::ByteReader;
use crate::dts::visitor::ItemAtCursor;
use crate::dts::{Diagnostic, FileType, HasSpan, Parser, Position, SeverityLevel, Span};
use itertools::Itertools;
use std::collections::HashMap;
use std::iter::empty;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, io};

#[derive(Default)]
pub struct ProjectFile {
    pub(crate) parser_diagnostics: Vec<Diagnostic>,
    pub(crate) analysis_diagnostics: Vec<Diagnostic>,
    pub(crate) file: Option<DtsFile>,
    pub(crate) context: Option<AnalysisContext>,
    pub(crate) file_type: FileType,
    pub(crate) source: String,
}

impl ProjectFile {
    pub fn includes(&self) -> Vec<PathBuf> {
        let Some(file) = &self.file else {
            return vec![];
        };
        file.elements
            .iter()
            .filter_map(|el| el.as_include())
            .map(|incl| incl.path())
            .collect_vec()
    }

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

    pub fn source(&self) -> &String {
        &self.source
    }
}

#[derive(Default)]
pub struct Project {
    files: HashMap<PathBuf, ProjectFile>,
}

impl Project {
    pub fn add_file(&mut self, file_name: PathBuf) -> Result<(), io::Error> {
        let content = fs::read_to_string(file_name.clone())?;
        let file_ending = FileType::from(file_name.as_path());
        self.add_file_with_text(file_name, content, file_ending);
        Ok(())
    }

    /// Adds a file to the project.
    /// Re-evaluates the file, if the file is already present.
    /// Does not re-evaluate dependencies, if they are cached.
    pub fn add_file_with_text(&mut self, path: PathBuf, text: String, file_type: FileType) {
        // First step: Parse file and all dependencies.
        // Dependencies are cached.
        self.parse_file(path.clone(), text, file_type);

        let keys = self.compute_key_order();

        for key in &keys {
            let proj_file = self.files.get(key).unwrap();
            let result = if let Some(file) = &proj_file.file {
                let mut analysis = Analysis::new(self);
                analysis.analyze_file(file, proj_file.file_type)
            } else {
                continue;
            };
            let proj_file = self.files.get_mut(key).unwrap();
            proj_file.context = Some(result.context);
            proj_file.analysis_diagnostics = result.diagnostics;
        }
    }

    /// Computes the order in which files must be analyzed.
    /// inefficient at the moment at O(n^3)
    fn compute_key_order(&self) -> Vec<PathBuf> {
        let mut map: HashMap<_, Vec<_>> = HashMap::new();
        for (path, file) in &self.files {
            let Some(dts_file) = &file.file else { continue };
            map.entry(path.clone()).or_default();
            let includes = dts_file
                .elements
                .iter()
                .filter_map(|el| el.as_include())
                .map(|incl| incl.path())
                .collect_vec();
            for include in includes {
                map.entry(include).or_default().push(path.clone())
            }
        }
        let mut current_order = self.files.keys().cloned().collect_vec();
        for (key, value) in &map {
            let key_idx = current_order.iter().position(|r| r == key).unwrap();
            for v in value {
                let value_idx = current_order.iter().position(|r| r == v).unwrap();
                if key_idx > value_idx {
                    current_order.swap(key_idx, value_idx)
                }
            }
        }
        current_order
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

    pub fn project_files(&self) -> impl Iterator<Item = &ProjectFile> {
        self.files.values()
    }

    pub fn get_analysis(&self, path: &PathBuf) -> Option<&AnalysisContext> {
        match self.files.get(path) {
            None => None,
            Some(project) => project.context.as_ref(),
        }
    }

    #[cfg(test)]
    pub fn assert_no_diagnostics(&self) {
        let diagnostics = self
            .files
            .values()
            .flat_map(|file| file.diagnostics())
            .collect_vec();
        if diagnostics.is_empty() {
            return;
        }
        for diag in diagnostics {
            println!("{diag:?}");
        }
        panic!("Found diagnostics")
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
        let reader = ByteReader::from_string(text.clone());
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
                        Err(err) => parser.diagnostics.push(Diagnostic::new(
                            include.span(),
                            include.source(),
                            DiagnosticKind::from(err),
                        )),
                    }
                }
                self.files.insert(
                    file_name,
                    ProjectFile {
                        parser_diagnostics: parser.diagnostics,
                        file: Some(file),
                        file_type,
                        context: None,
                        source: text,
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
                        source: text,
                        context: None,
                        file_type,
                    },
                );
            }
        };
    }

    pub fn get_file(&self, path: &Path) -> Option<&ProjectFile> {
        self.files.get(path)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::diagnostics::DiagnosticKind;
    use crate::dts::lexer::TokenKind;
    use crate::dts::test::Code;
    use crate::dts::{ast, Diagnostic, FileType, HasSpan, ItemAtCursor, Project};
    use itertools::Itertools;
    use std::error::Error;
    use std::io::{Seek, Write};
    use std::path::PathBuf;

    #[test]
    pub fn file_with_includes() {
        let mut project = Project::default();
        let code = "";
        let path: PathBuf = "path/to/file.dtsi".into();
        project.add_file_with_text(path.clone(), code.to_owned(), FileType::DtSourceInclude);

        let code2 = r#"
/dts-v1/;

/include/ "path/to/file.dtsi"
"#;
        let path2: PathBuf = "path/to/other/file.dts".into();
        project.add_file_with_text(path2.clone(), code2.to_owned(), FileType::DtSource);
        assert!(project.get_file(&path).is_some());
        assert!(project.get_file(&path2).is_some());
        assert_eq!(project.get_diagnostics(&path).next(), None);
        assert_eq!(project.get_diagnostics(&path2).next(), None);
        assert!(project.get_root(&path).is_some());
        assert!(project.get_analysis(&path).is_some());
        assert!(project.get_analysis(&path2).is_some());
    }

    #[test]
    pub fn cross_file_references() {
        let mut project = Project::default();
        let code1 = Code::new(
            r#"
/ {
    some_node: node_a {
        // ...
    };
};
"#,
        );
        let mut file1 = tempfile::NamedTempFile::new().expect("cannot create temporary file");
        write!(file1, "{}", code1.code()).expect("write dont work");
        file1.rewind().expect("rewind dont work");

        let code2 = Code::new(
            format!(
                r#"
/dts-v1/;

/include/ "{}"

&some_node {{
}};
"#,
                file1.path().to_string_lossy()
            )
            .as_str(),
        );

        let mut file2 = tempfile::NamedTempFile::new().expect("cannot create temporary file");
        write!(file2, "{}", code2.code()).expect("write dont work");
        file2.rewind().expect("rewind dont work");

        project
            .add_file(file2.path().to_path_buf())
            .expect("Unexpected IO error");

        project.assert_no_diagnostics();

        let substr = code2.s1("&some_node");
        let item = project
            .find_at_pos(&file2.path().to_path_buf(), &substr.span().start())
            .expect("Found no item");
        let ItemAtCursor::Reference(reference) = item else {
            panic!("Found non-node at cursor")
        };
        assert_eq!(reference, &ast::Reference::Label("some_node".to_owned()));
        match project.get_node_position(&file2.path().to_path_buf(), reference) {
            Some((span, path)) => {
                assert_eq!(span, code1.s1("node_a").span());
                assert_eq!(path.to_path_buf(), file1.path().to_path_buf());
            }
            None => panic!("References does not reference nodes"),
        }
    }

    #[test]
    // We don't push an 'errors in include' anymore. Maybe later.
    #[ignore]
    pub fn error_in_included_file_add_include_before_dts() {
        let mut project = Project::default();
        let code = Code::new("/ {}"); // missing semicolon
        let path: PathBuf = "path/to/file.dtsi".into();
        project.add_file_with_text(
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
        project.add_file_with_text(path2.clone(), code2.code().to_owned(), FileType::DtSource);

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
