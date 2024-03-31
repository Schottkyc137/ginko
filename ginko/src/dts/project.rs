use crate::dts::analysis::AnalysisContext;
use crate::dts::ast::{DtsFile, Reference};
use crate::dts::data::HasSource;
use crate::dts::importer::{CyclicDependencyChecker, CyclicDependencyError};
use crate::dts::lexer::Lexer;
use crate::dts::reader::ByteReader;
use crate::dts::visitor::ItemAtCursor;
use crate::dts::{Analysis, Diagnostic, FileType, HasSpan, Parser, Position, SeverityLevel, Span};
use itertools::Itertools;
use std::collections::HashMap;
use std::iter::empty;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    pub(crate) parser_diagnostics: Vec<Diagnostic>,
    pub(crate) analysis_diagnostics: Vec<Diagnostic>,
    pub(crate) file: Option<DtsFile>,
    pub(crate) context: Option<AnalysisContext>,
    pub(crate) file_type: FileType,
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
    importer: CyclicDependencyChecker<PathBuf>,
}

impl Project {
    pub fn add_file(&mut self, path: PathBuf, text: String, file_type: FileType) {
        let project_file = self.analyze_text(text, path.clone(), file_type);
        self.files.insert(path.clone(), project_file);
        let dependencies = self.importer.dependencies_of(path.clone()).collect_vec();
        for dependency in dependencies {
            self.reanalyze_file(&dependency)
        }
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
                let mut analyis_diagnostics = Vec::new();
                let mut analysis = Analysis::new(file_type, self);
                analysis.analyze_file(&mut analyis_diagnostics, &file);
                ProjectFile {
                    analysis_diagnostics: analyis_diagnostics,
                    parser_diagnostics: parser.diagnostics,
                    file: Some(file),
                    context: Some(analysis.into_context()),
                    file_type,
                }
            }
            Err(err) => ProjectFile {
                parser_diagnostics: vec![err],
                analysis_diagnostics: vec![],
                file: None,
                context: None,
                file_type,
            },
        }
    }

    fn reanalyze_file(&mut self, path: &PathBuf) {
        let Some(proj_file) = self.files.get(path) else {
            return;
        };
        let Some(file) = &proj_file.file else {
            return;
        };

        struct ReAnalysisManager<'a> {
            project: &'a Project,
        }

        impl FileManager for ReAnalysisManager<'_> {
            fn get_file(&self, path: &Path) -> Option<&ProjectFile> {
                self.project.get_file(path)
            }

            fn add_file_with_parent(
                &mut self,
                _path: PathBuf,
                _parent: Option<PathBuf>,
                _text: String,
                _file_type: FileType,
            ) -> Result<&ProjectFile, CyclicDependencyError<PathBuf>> {
                unreachable!("All files should be present when re-analyzing")
            }
        }

        let mut mgr = ReAnalysisManager { project: self };
        let mut analysis = Analysis::new(proj_file.file_type, &mut mgr);
        let mut diagnostics = Vec::new();
        analysis.analyze_file(&mut diagnostics, file);
        let context = analysis.into_context();
        let mut_proj_file = self.files.get_mut(path).expect("File suddenly disappeared");
        mut_proj_file.context = Some(context);
        mut_proj_file.analysis_diagnostics = diagnostics;
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
    use itertools::Itertools;
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
