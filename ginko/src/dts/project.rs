use crate::dts::analysis::AnalysisContext;
use crate::dts::ast::{DtsFile, Reference};
use crate::dts::lexer::Lexer;
use crate::dts::reader::ByteReader;
use crate::dts::visitor::ItemAtCursor;
use crate::dts::{Analysis, Diagnostic, FileType, HasSpan, Parser, Position, Span};
use std::collections::HashMap;
use std::path::PathBuf;

struct ProjectFile {
    diagnostics: Vec<Diagnostic>,
    file: Option<DtsFile>,
    context: Option<AnalysisContext>,
}

#[derive(Default)]
pub struct Project {
    files: HashMap<PathBuf, ProjectFile>,
}

static NO_DIAGNOSTICS: Vec<Diagnostic> = Vec::new();

impl Project {
    pub fn add_file(&mut self, path: PathBuf, text: String, file_type: FileType) {
        let file = analyze_text(text, path.clone(), file_type);
        self.files.insert(path, file);
    }

    pub fn remove_file(&mut self, path: &PathBuf) {
        self.files.remove(path);
    }

    pub fn get_diagnostics(&self, path: &PathBuf) -> &[Diagnostic] {
        self.files
            .get(path)
            .map(|file| &file.diagnostics)
            .unwrap_or(&NO_DIAGNOSTICS)
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
        let Some(analysis) = self.get_analysis(path) else {
            return None;
        };
        let Some(referenced) = analysis.get_referenced(reference) else {
            return None;
        };
        Some(format!("Node {}", referenced.name.name.clone()))
    }

    pub fn get_node_position(&self, path: &PathBuf, reference: &Reference) -> Option<Span> {
        let Some(analysis) = self.get_analysis(path) else {
            return None;
        };
        let Some(referenced) = analysis.get_referenced(reference) else {
            return None;
        };
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
}

fn analyze_text(text: String, file_name: PathBuf, file_type: FileType) -> ProjectFile {
    let reader = ByteReader::from_string(text);
    let lexer = Lexer::new(reader, file_name.into());
    let mut parser = Parser::new(lexer);
    match parser.file() {
        Ok(file) => {
            let mut analysis = Analysis::new(file_type);
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
