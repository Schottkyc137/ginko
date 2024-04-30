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
        let file_name = file_name.canonicalize()?;
        let content = fs::read_to_string(file_name.clone())?;
        let file_ending = FileType::from(file_name.as_path());
        self.add_file_with_text(file_name, content, file_ending);
        Ok(())
    }

    /// Adds a file to the project with already given text.
    /// Re-evaluates the file, if the file is already present.
    /// Does not re-evaluate dependencies, if they are cached.
    ///
    /// # Parameters
    /// * file_name: The name of the file.
    /// * text: The contents of the file.
    /// * file_type: Defines how the file should be analyzed.
    ///
    /// # Panics
    /// If `file_name` does not point to a valid file.
    pub fn add_file_with_text(&mut self, file_name: PathBuf, text: String, file_type: FileType) {
        let file_name = file_name.canonicalize().expect("File must be present");
        // First step: Parse file and all dependencies.
        // Dependencies are cached.
        self.parse_file(file_name.clone(), text, file_type);

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
            for include in includes.into_iter().flatten() {
                map.entry(include).or_default().push(path.clone())
            }
        }
        // This is very inefficient. Probably there is a better way.
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

    pub fn remove_file(&mut self, path: &Path) {
        if let Ok(path) = path.canonicalize() {
            self.files.remove(&path);
        }
    }

    pub fn get_diagnostics(&self, path: &Path) -> Box<dyn Iterator<Item = &Diagnostic> + '_> {
        let Some(file) = self.get_file(path) else {
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

    pub fn get_analysis(&self, path: &Path) -> Option<&AnalysisContext> {
        match self.get_file(path) {
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

    pub fn find_at_pos<'a>(&'a self, path: &Path, position: &Position) -> Option<ItemAtCursor<'a>> {
        let file = match self.get_file(path).and_then(|file| file.file.as_ref()) {
            None => return None,
            Some(file) => file,
        };
        file.item_at_cursor(position)
    }

    pub fn document_reference(&self, path: &Path, reference: &Reference) -> Option<String> {
        let referenced = self.get_analysis(path)?.get_referenced(reference)?;
        Some(format!("Node {}", referenced.name.name.clone()))
    }

    pub fn get_node_position(
        &self,
        path: &Path,
        reference: &Reference,
    ) -> Option<(Span, Arc<Path>)> {
        let referenced = self.get_analysis(path)?.get_referenced(reference)?;
        Some((referenced.name.span(), referenced.name.source()))
    }

    pub fn get_root(&self, path: &Path) -> Option<&DtsFile> {
        match self.get_file(path) {
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
                    let canonicalized_path = match include.path() {
                        Ok(path) => path,
                        Err(err) => {
                            parser.diagnostics.push(Diagnostic::new(
                                include.span(),
                                include.source(),
                                DiagnosticKind::from(err),
                            ));
                            continue;
                        }
                    };
                    // Avoids duplicate insertion and cyclic dependencies
                    if self.files.contains_key(&canonicalized_path) {
                        continue;
                    }
                    match fs::read_to_string(&canonicalized_path) {
                        Ok(text) => {
                            let typ = FileType::from(canonicalized_path.as_path());
                            self.parse_file(canonicalized_path, text, typ);
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
        match path.canonicalize() {
            Ok(path) => self.files.get(&path),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::diagnostics::DiagnosticKind;
    use crate::dts::lexer::TokenKind;
    use crate::dts::test::Code;
    use crate::dts::{ast, Diagnostic, FileType, HasSpan, ItemAtCursor, Project};
    use itertools::Itertools;
    use std::io::{Seek, Write};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn tempfile(code: &str) -> (Code, NamedTempFile) {
        let code = Code::new(code);
        let mut file = NamedTempFile::new().expect("cannot create temporary file");
        write!(file, "{}", code.code()).expect("write dont work");
        file.rewind().expect("rewind dont work");
        (code, file)
    }

    #[test]
    pub fn file_with_includes() {
        let mut project = Project::default();
        let (code, file) = tempfile("");
        project.add_file_with_text(
            file.path().to_owned(),
            code.code().to_owned(),
            FileType::DtSourceInclude,
        );

        let (code2, file2) = tempfile(
            format!(
                r#"
/dts-v1/;

/include/ "{}"
"#,
                file.path().display()
            )
            .as_str(),
        );
        project.add_file_with_text(
            file2.path().to_owned(),
            code2.code().to_owned(),
            FileType::DtSource,
        );
        assert!(project.get_file(file.path()).is_some());
        assert!(project.get_file(file2.path()).is_some());
        assert_eq!(project.get_diagnostics(file.path()).next(), None);
        assert_eq!(project.get_diagnostics(file2.path()).next(), None);
        assert!(project.get_root(file.path()).is_some());
        assert!(project.get_analysis(file.path()).is_some());
        assert!(project.get_analysis(file2.path()).is_some());
    }

    #[test]
    pub fn cross_file_references() {
        let mut project = Project::default();
        let (code1, file1) = tempfile(
            r#"
/ {
    some_node: node_a {
        // ...
    };
};
"#,
        );
        let (code2, file2) = tempfile(
            format!(
                r#"
/dts-v1/;

/include/ "{}"

&some_node {{
}};
"#,
                file1.path().display()
            )
            .as_str(),
        );

        project
            .add_file(file2.path().to_path_buf())
            .expect("Unexpected IO error");

        project.assert_no_diagnostics();

        let substr = code2.s1("&some_node");
        let item = project
            .find_at_pos(file2.path(), &substr.span().start())
            .expect("Found no item");
        let ItemAtCursor::Reference(reference) = item else {
            panic!("Found non-node at cursor")
        };
        assert_eq!(reference, &ast::Reference::Label("some_node".to_owned()));
        match project.get_node_position(file2.path(), reference) {
            Some((span, path)) => {
                assert_eq!(span, code1.s1("node_a").span());
                assert_eq!(
                    path.to_path_buf(),
                    file1.path().canonicalize().expect("File does not exist")
                );
            }
            None => panic!("References does not reference nodes"),
        }
    }

    #[test]
    pub fn file_with_multiple_includes() {
        let mut project = Project::default();
        let (_, file1) = tempfile("");
        project
            .add_file(file1.path().to_path_buf())
            .expect("Unexpected IO error");

        let (_, file2) = tempfile("");
        project
            .add_file(file2.path().to_path_buf())
            .expect("Unexpected IO error");

        let (_, file3) = tempfile(
            format!(
                r#"
/dts-v1/;

/include/ "{}"
/include/ "{}"
"#,
                file1.path().display(),
                file2.path().display()
            )
            .as_str(),
        );

        project
            .add_file(file3.path().to_path_buf())
            .expect("Unexpected IO error");

        project.assert_no_diagnostics();
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
