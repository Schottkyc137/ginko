use crate::dts::analysis::AnalysisContext;
use crate::dts::ast::{DtsFile, Reference};
use crate::dts::lexer::Lexer;
use crate::dts::reader::ByteReader;
use crate::dts::visitor::ItemAtCursor;
use crate::dts::{Analysis, Diagnostic, FileType, HasSpan, Parser, Position, Span};
use std::collections::HashMap;
use url::Url;

struct ProjectFile {
    diagnostics: Vec<Diagnostic>,
    file: Option<DtsFile>,
    context: Option<AnalysisContext>,
}

#[derive(Default)]
pub struct Project {
    files: HashMap<Url, ProjectFile>,
}

static NO_DIAGNOSTICS: Vec<Diagnostic> = Vec::new();

impl Project {
    pub fn add_file(&mut self, url: Url, text: String, file_type: FileType) {
        let (diagnostics, file, context) = analyze_text(text.clone(), file_type);
        self.files.insert(
            url.clone(),
            ProjectFile {
                diagnostics,
                file,
                context,
            },
        );
    }

    pub fn remove_file(&mut self, url: &Url) {
        self.files.remove(url);
    }

    pub fn get_diagnostics(&self, url: &Url) -> &[Diagnostic] {
        self.files
            .get(url)
            .map(|file| &file.diagnostics)
            .unwrap_or(&NO_DIAGNOSTICS)
    }

    pub fn get_analysis(&self, url: &Url) -> Option<&AnalysisContext> {
        match self.files.get(url) {
            None => None,
            Some(project) => project.context.as_ref(),
        }
    }

    pub fn find_at_pos<'a>(&'a self, url: &Url, position: &Position) -> Option<ItemAtCursor<'a>> {
        let file = match self.files.get(url).and_then(|file| file.file.as_ref()) {
            None => return None,
            Some(file) => file,
        };
        file.item_at_cursor(position)
    }

    pub fn document_reference(&self, uri: &Url, reference: &Reference) -> Option<String> {
        let Some(analysis) = self.get_analysis(uri) else {
            return None;
        };
        let Some(referenced) = analysis.get_referenced(reference) else {
            return None;
        };
        Some(format!("Node {}", referenced.name.name.clone()))
    }

    pub fn get_node_position(&self, uri: &Url, reference: &Reference) -> Option<Span> {
        let Some(analysis) = self.get_analysis(uri) else {
            return None;
        };
        let Some(referenced) = analysis.get_referenced(reference) else {
            return None;
        };
        Some(referenced.name.span())
    }

    pub fn get_root(&self, uri: &Url) -> Option<&DtsFile> {
        match self.files.get(uri) {
            Some(ProjectFile {
                file: Some(file), ..
            }) => Some(file),
            _ => None,
        }
    }
}

fn analyze_text(
    text: String,
    file_type: FileType,
) -> (Vec<Diagnostic>, Option<DtsFile>, Option<AnalysisContext>) {
    let reader = ByteReader::from_string(text);
    let lexer = Lexer::new(reader);
    let mut parser = Parser::new(lexer);
    match parser.file() {
        Ok(file) => {
            let mut analysis = Analysis::new(file_type);
            analysis.analyze_file(&mut parser.diagnostics, &file);
            (
                parser.diagnostics,
                Some(file),
                Some(analysis.into_context()),
            )
        }
        Err(err) => (vec![err], None, None),
    }
}
