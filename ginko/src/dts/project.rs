use crate::dts::analysis::{Analysis, AnalysisContext};
use crate::dts::ast::{DtsFile, Include, Reference};
use crate::dts::data::HasSource;
use crate::dts::error_codes::SeverityMap;
use crate::dts::reader::ByteReader;
use crate::dts::tokens::Lexer;
use crate::dts::visitor::ItemAtCursor;
use crate::dts::{Diagnostic, FileType, HasSpan, Parser, ParserContext, Position, Severity, Span};
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
    pub fn parsed(
        diagnostics: Vec<Diagnostic>,
        file: DtsFile,
        file_type: FileType,
        source: String,
    ) -> ProjectFile {
        ProjectFile {
            parser_diagnostics: diagnostics,
            file: Some(file),
            file_type,
            context: None,
            source,
            analysis_diagnostics: vec![],
        }
    }

    pub fn unrecoverable(err: Diagnostic, source: String, file_type: FileType) -> ProjectFile {
        ProjectFile {
            parser_diagnostics: vec![err],
            analysis_diagnostics: vec![],
            file: None,
            source,
            context: None,
            file_type,
        }
    }

    pub fn analysis_context(&self) -> Option<&AnalysisContext> {
        self.context.as_ref()
    }

    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.parser_diagnostics
            .iter()
            .chain(&self.analysis_diagnostics)
    }

    pub fn has_errors(&self, severity_map: &SeverityMap) -> bool {
        self.diagnostics()
            .any(|diagnostic| diagnostic.severity(severity_map) == Severity::Error)
    }

    pub fn source(&self) -> &String {
        &self.source
    }
}

#[derive(Default)]
pub struct Project {
    files: HashMap<PathBuf, ProjectFile>,
    pub include_paths: Vec<PathBuf>,
    pub severities: SeverityMap,
}

impl Project {
    pub fn set_include_paths(&mut self, include_paths: Vec<String>) {
        self.include_paths = include_paths
            .iter()
            .map(|path| dunce::canonicalize(path).unwrap_or_default())
            .collect::<Vec<PathBuf>>();
    }

    pub fn add_file(&mut self, file_name: String) -> Result<(), io::Error> {
        let file_name = dunce::canonicalize(file_name)?;
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
        let file_name = dunce::canonicalize(file_name).expect("File must be present");
        // First step: Parse file and all dependencies.
        // Dependencies are cached.
        self.parse_file(file_name.clone(), text, file_type);

        let keys = self.compute_key_order();

        let mut analysis = Analysis::new();
        for key in &keys {
            let proj_file = self.files.get(key).unwrap();
            let result = if let Some(file) = &proj_file.file {
                analysis.analyze_file(file, proj_file.file_type, self)
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
        if let Ok(path) = dunce::canonicalize(path) {
            self.files.remove(&path);
        }
    }

    pub fn get_diagnostics(&self, path: &Path) -> Box<dyn Iterator<Item = &Diagnostic> + '_> {
        let Some(file) = self.get_file(path) else {
            return Box::new(empty());
        };
        Box::new(file.diagnostics())
    }

    pub fn all_diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.files.values().flat_map(|file| file.diagnostics())
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
        // add the file's directory to the include paths to allow local includes
        let mut include_paths = self.include_paths.clone();
        if let Some(parent) = file_name.parent() {
            include_paths.insert(0, parent.into());
        }
        let mut parser = Parser::new(lexer, ParserContext { include_paths });
        match parser.file() {
            Ok(file) => {
                // insert dummy file to be defined so that no cyclic dependency can occur.
                self.files.insert(file_name.clone(), ProjectFile::default());
                file.elements
                    .iter()
                    .filter_map(|primary| primary.as_include())
                    .for_each(|include| self.parse_included_file(&mut parser.diagnostics, include));
                self.files.insert(
                    file_name,
                    ProjectFile::parsed(parser.diagnostics, file, file_type, text),
                );
            }
            Err(err) => {
                self.files
                    .insert(file_name, ProjectFile::unrecoverable(err, text, file_type));
            }
        };
    }

    fn parse_included_file(&mut self, diagnostics: &mut Vec<Diagnostic>, include: &Include) {
        let canonicalized_path = match include.path() {
            Ok(path) => path,
            Err(err) => {
                diagnostics.push(Diagnostic::io_error(include.span(), include.source(), err));
                return;
            }
        };
        // Avoids duplicate insertion and cyclic dependencies
        if self.files.contains_key(&canonicalized_path) {
            return;
        }
        match fs::read_to_string(&canonicalized_path) {
            Ok(text) => {
                let typ = FileType::from(canonicalized_path.as_path());
                self.parse_file(canonicalized_path, text, typ);
            }
            Err(err) => {
                diagnostics.push(Diagnostic::io_error(include.span(), include.source(), err))
            }
        }
    }

    pub fn get_file(&self, path: &Path) -> Option<&ProjectFile> {
        match dunce::canonicalize(path) {
            Ok(path) => self.files.get(&path),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
// For some reason, this fails under windows with error "The system cannot find the file specified. (os error 2)"
#[cfg(not(windows))]
mod tests {
    use crate::dts::error_codes::ErrorCode;
    use crate::dts::test::Code;
    use crate::dts::tokens::TokenKind;
    use crate::dts::{ast, Diagnostic, HasSpan, ItemAtCursor, Project};
    use assert_matches::assert_matches;
    use itertools::Itertools;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    struct TempDir {
        pub inner: tempfile::TempDir,
    }

    impl TempDir {
        pub fn new() -> TempDir {
            let dir = tempdir().expect("Cannot create temporary directory");
            TempDir { inner: dir }
        }

        pub fn add_file(
            &self,
            name: impl AsRef<Path>,
            content: impl AsRef<str>,
        ) -> (Code, PathBuf) {
            let code = Code::new(content.as_ref());
            let file_path = self.inner.path().join(name);

            fs::write(&file_path, code.code()).expect("Cannot write to file");
            (code, file_path)
        }

        pub fn new_file(&self, name: impl AsRef<Path>) -> (File, PathBuf) {
            let file_path = self.inner.path().join(name);
            (
                File::create(&file_path).expect("Cannot create temporary file"),
                file_path,
            )
        }
    }

    #[test]
    pub fn file_with_includes() {
        let mut project = Project::default();
        let temp_dir = TempDir::new();
        let (_, path1) = temp_dir.add_file("tests-include.dtsi", "");
        project
            .add_file(path1.clone().into_os_string().into_string().unwrap())
            .expect("Cannot add file");

        let (_, file2) = temp_dir.add_file(
            "tests-file.dts",
            format!(
                r#"
/dts-v1/;

/include/ "{}"
"#,
                path1.display()
            ),
        );
        project
            .add_file(file2.clone().into_os_string().into_string().unwrap())
            .expect("Cannot add file");
        assert!(project.get_file(&path1).is_some());
        assert!(project.get_file(&file2).is_some());
        assert_eq!(project.get_diagnostics(&path1).next(), None);
        assert_eq!(project.get_diagnostics(&file2).next(), None);
        assert!(project.get_root(&path1).is_some());
        assert!(project.get_analysis(&path1).is_some());
        assert!(project.get_analysis(&file2).is_some());
    }

    #[test]
    pub fn cross_file_references() {
        let mut project = Project::default();
        let temp_dir = TempDir::new();
        let (code1, file1) = temp_dir.add_file(
            "tests-include.dtsi",
            r#"
/ {
    some_node: node_a {
        // ...
    };
};
"#,
        );
        let (code2, file2) = temp_dir.add_file(
            "tests-file.dts",
            format!(
                r#"
/dts-v1/;

/include/ "{}"

&some_node {{
}};

&{{/node_a}} {{
}};
"#,
                file1.display()
            ),
        );

        project
            .add_file(file2.clone().into_os_string().into_string().unwrap())
            .expect("Unexpected IO error");

        project.assert_no_diagnostics();

        let substr = code2.s1("&some_node");
        let item = project
            .find_at_pos(&file2, &substr.span().start())
            .expect("Found no item");
        let ItemAtCursor::Reference(reference) = item else {
            panic!("Found non-node at cursor")
        };
        assert_eq!(reference, &ast::Reference::Label("some_node".to_owned()));
        match project.get_node_position(&file2, reference) {
            Some((span, path)) => {
                assert_eq!(span, code1.s1("node_a").span());
                assert_eq!(
                    path.to_path_buf(),
                    dunce::canonicalize(&file1).expect("File does not exist")
                );
            }
            None => panic!("References does not reference nodes"),
        }

        let substr = code2.s1("&{/node_a}");
        let item = project
            .find_at_pos(&file2, &substr.span().start())
            .expect("Found no item");
        let ItemAtCursor::Reference(reference) = item else {
            panic!("Found non-node at cursor")
        };
        assert_eq!(reference, &ast::Reference::Path("/node_a".into()));
        match project.get_node_position(&file2, reference) {
            Some((span, path)) => {
                assert_eq!(span, code1.s1("node_a").span());
                assert_eq!(
                    path.to_path_buf(),
                    dunce::canonicalize(&file1).expect("File does not exist")
                );
            }
            None => panic!("References does not reference nodes"),
        }
    }

    #[test]
    pub fn cyclic_import_error() {
        let temp_dir = TempDir::new();
        let (mut file1, path1) = temp_dir.new_file("test.dts");
        let (mut file2, path2) = temp_dir.new_file("tests-include.dtsi");

        write!(file1, r#"/dts-v1/; /include/ "{}""#, path2.display())
            .expect("Cannot write to file 2");
        write!(file2, r#"/include/ "{}""#, path1.display()).expect("Cannot write to file1");

        let mut project = Project::default();
        project
            .add_file(path1.into_os_string().into_string().unwrap())
            .expect("Cannot add file to project");

        let diag = project.all_diagnostics().cloned().collect_vec();
        // This is explicitly vague. There should be some error somewhere,
        // but the exact location is not perfect at the moment.
        // This is because the error only occurs in one file while it should occur in all
        // files affected by the cyclic include.
        assert_matches!(
            &diag[..],
            &[Diagnostic {
                kind: ErrorCode::CyclicDependencyError,
                ..
            }]
        );
    }

    #[test]
    pub fn file_with_multiple_includes() {
        let mut project = Project::default();
        let temp_dir = TempDir::new();
        let (_, file1) = temp_dir.add_file("test1.dtsi", "");
        project
            .add_file(file1.clone().into_os_string().into_string().unwrap())
            .expect("Cannot add file");
        let (_, file2) = temp_dir.add_file("test2.dtsi", "");
        project
            .add_file(file2.clone().into_os_string().into_string().unwrap())
            .expect("Cannot add file");
        let (_, file3) = temp_dir.add_file(
            "test.dts",
            format!(
                r#"
/dts-v1/;

/include/ "{}"
/include/ "{}"
"#,
                file1.display(),
                file2.display()
            ),
        );

        project
            .add_file(file3.into_os_string().into_string().unwrap())
            .expect("Unexpected IO error");
        project.assert_no_diagnostics();
    }

    #[test]
    pub fn file_with_nested_includes() {
        let mut project = Project::default();
        let temp_dir = TempDir::new();
        let (_, file1) = temp_dir.add_file("tests-include1.dtsi", "");
        let (_, file2) = temp_dir.add_file(
            "tests-include2.dtsi",
            format!(r#"/include/ "{}""#, file1.display()).as_str(),
        );
        let (_, file3) = temp_dir.add_file(
            "test.dts",
            format!(
                r#"
/dts-v1/;

/include/ "{}"
"#,
                file2.display()
            ),
        );

        project
            .add_file(file3.clone().into_os_string().into_string().unwrap())
            .expect("Unexpected IO error");

        project.assert_no_diagnostics();

        assert!(project.get_file(&file1).is_some());
        assert!(project.get_file(&file2).is_some());
        assert!(project.get_file(&file3).is_some());
    }

    #[test]
    pub fn error_in_included_file_add_include_before_dts() {
        let mut project = Project::default();
        let temp_dir = TempDir::new();
        let (code1, file1) = temp_dir.add_file("error.dtsi", "/ {}"); // missing semicolon
        let (code2, file2) = temp_dir.add_file(
            "test.dts",
            format!(
                r#"
/dts-v1/;

/include/ "{}"
"#,
                file1.display()
            ),
        );
        project
            .add_file(file2.clone().into_os_string().into_string().unwrap())
            .expect("Cannot add file to project");

        assert!(project.get_file(&file1).is_some());
        assert!(project.get_file(&file2).is_some());
        assert_eq!(
            project.get_diagnostics(&file1).cloned().collect_vec(),
            vec![Diagnostic::expected(
                code1.s1("}").end().as_span(),
                dunce::canonicalize(&file1)
                    .expect("Cannot canonicalize")
                    .into(),
                &[TokenKind::Semicolon]
            )]
        );
        assert_eq!(
            project.get_diagnostics(&file2).cloned().collect_vec(),
            vec![Diagnostic::new(
                code2
                    .s1(format!(r#"/include/ "{}""#, file1.display()).as_str())
                    .span(),
                dunce::canonicalize(file2)
                    .expect("Cannot canonicalize")
                    .into(),
                ErrorCode::ErrorsInInclude,
                "Included file contains errors"
            )]
        );
    }

    #[test]
    pub fn file_with_include_paths_includes() {
        let includes_dir = TempDir::new();
        let (_, file1) = includes_dir.add_file("tests-include1.dtsi", "");
        let another_includes_dir = TempDir::new();
        let (_, file2) = another_includes_dir.add_file("tests-include2.dtsi", "");

        let temp_dir = TempDir::new();
        let (_, file3) = temp_dir.add_file(
            "test.dts",
            r#"
/dts-v1/;

/include/ "tests-include1.dtsi"
/include/ "tests-include2.dtsi"
"#,
        );

        let mut project = Project::default();
        let include_paths = vec![
            includes_dir.inner.path().display().to_string(),
            another_includes_dir.inner.path().display().to_string(),
        ];

        project.set_include_paths(include_paths);

        project
            .add_file(file3.clone().into_os_string().into_string().unwrap())
            .expect("Unexpected IO error");

        project.assert_no_diagnostics();

        assert!(project.get_file(&file1).is_some());
        assert!(project.get_file(&file2).is_some());
        assert!(project.get_file(&file3).is_some());
    }
}
