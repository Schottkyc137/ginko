use crate::dts::ast::{Cast, Include};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::lex::lex::lex;
use crate::dts::syntax::Parser;
use crate::dts::FileType;
use crate::dts::{ast, model};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use std::mem::{replace, take};
use std::path::PathBuf;

mod cell;
mod file;
mod node;
mod property;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum BitWidth {
    W8,
    W16,
    #[default]
    W32,
    W64,
}

pub enum BitWidthConversionError {
    Illegal(u32),
}

impl TryFrom<u32> for BitWidth {
    type Error = BitWidthConversionError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        use BitWidth::*;
        Ok(match value {
            8 => W8,
            16 => W16,
            32 => W32,
            64 => W64,
            other => return Err(BitWidthConversionError::Illegal(other)),
        })
    }
}

#[derive(Default)]
pub struct AnalysisContext {
    bit_width: BitWidth,
    file_type: FileType,
}

impl AnalysisContext {
    pub fn with_bit_width(&self, width: BitWidth) -> AnalysisContext {
        AnalysisContext {
            bit_width: width,
            ..Default::default()
        }
    }
}

pub trait Analysis<T> {
    fn analyze(
        &self,
        context: &AnalysisContext,
        project: &RefCell<ProjectState>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<T, Diagnostic>;
}

pub trait PushIntoDiagnostics<T> {
    fn or_push_into(self, diagnostics: &mut Vec<Diagnostic>) -> Option<T>;
}

impl<T> PushIntoDiagnostics<T> for Result<T, Diagnostic> {
    fn or_push_into(self, diagnostics: &mut Vec<Diagnostic>) -> Option<T> {
        match self {
            Ok(val) => Some(val),
            Err(err) => {
                diagnostics.push(err);
                None
            }
        }
    }
}

#[derive(Default, Debug)]
pub struct Project {
    state: ProjectState,
}

impl Project {
    pub fn add_file(&mut self, location: PathBuf, contents: &str) {
        let tokens = lex(contents);
        let parser = Parser::new(tokens.into_iter());
        let (node, diagnostics) = parser.parse(Parser::parse_file);
        let mut file = ProjectFile {
            path: Some(location.clone()),
            source: contents.to_string(),
            ast: ast::File::cast(node).unwrap(),
            syntax_diagnostics: diagnostics,
            model: None,
            analysis_diagnostics: Vec::new(),
        };
        let context = AnalysisContext::default();
        let mut diagnostics = Vec::new();

        let state = RefCell::new(take(&mut self.state));

        let model = file
            .ast
            .analyze(&context, &state, &mut diagnostics)
            .or_push_into(&mut diagnostics);

        let _ = replace(&mut self.state, state.into_inner());
        file.model = model;
        self.state.insert(location, file);
    }

    pub fn add_file_from_fs(&mut self, location: PathBuf) -> Result<(), io::Error> {
        let contents = std::fs::read_to_string(&location)?;
        self.add_file(location, &contents);
        Ok(())
    }

    pub fn add_include_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.state.include_paths.extend(paths);
    }

    pub fn project_files(&self) -> impl Iterator<Item = &ProjectFile> {
        self.state.files.values()
    }
}

#[derive(Debug)]
pub struct ProjectFile {
    path: Option<PathBuf>,
    source: String,
    ast: ast::File,
    syntax_diagnostics: Vec<Diagnostic>,
    model: Option<model::File>,
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

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }
}

#[derive(Default, Debug)]
pub struct ProjectState {
    files: HashMap<PathBuf, ProjectFile>,
    include_paths: Vec<PathBuf>,
}

impl ProjectState {
    pub fn new() -> ProjectState {
        ProjectState::default()
    }

    pub fn get_or_insert(&mut self, path: PathBuf) -> Result<&ProjectFile, io::Error> {
        if !self.files.contains_key(&path) {
            let contents = std::fs::read_to_string(&path)?;
            let tokens = lex(&contents);
            let parser = Parser::new(tokens.into_iter());
            let (node, diagnostics) = parser.parse(Parser::parse_file);
            self.files.insert(
                path.clone(),
                ProjectFile {
                    path: Some(path.clone()),
                    source: contents,
                    ast: ast::File::cast(node).unwrap(),
                    syntax_diagnostics: diagnostics,
                    model: None,
                    analysis_diagnostics: Vec::new(),
                },
            );
        }
        Ok(self.files.get(&path).unwrap())
    }
}

impl ProjectState {
    pub fn insert(&mut self, key: PathBuf, value: ProjectFile) {
        self.files.insert(key, value);
    }

    pub fn get(&self, path: &PathBuf) -> Option<&ProjectFile> {
        self.files.get(path)
    }

    pub fn get_mut(&mut self, path: &PathBuf) -> Option<&mut ProjectFile> {
        self.files.get_mut(path)
    }
}

impl Include {
    pub fn resolve<'a>(
        &self,
        project: &'a mut ProjectState,
    ) -> Option<Result<&'a ProjectFile, io::Error>> {
        let target: PathBuf = self.target()?.into();
        Some(project.get_or_insert(target))
    }
}

#[cfg(test)]
pub trait NoErrorAnalysis<T> {
    fn analyze_no_errors(&self) -> T;
}

#[cfg(test)]
impl<I, T> NoErrorAnalysis<T> for I
where
    I: Analysis<T>,
{
    fn analyze_no_errors(&self) -> T {
        let mut diagnostics = Vec::new();
        let context = AnalysisContext::default();
        let state = RefCell::default();
        let result = self.analyze(&context, &state, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty());
        result
    }
}

#[cfg(test)]
pub trait ExpectedErrorAnalysis<T> {
    fn analyze_exp_error(&self) -> Diagnostic;
}

#[cfg(test)]
impl<I, T> ExpectedErrorAnalysis<T> for I
where
    I: Analysis<T>,
    T: Debug,
{
    fn analyze_exp_error(&self) -> Diagnostic {
        let mut diagnostics = Vec::new();
        let context = AnalysisContext::default();
        let state = RefCell::default();
        let result = self
            .analyze(&context, &state, &mut diagnostics)
            .unwrap_err();
        assert!(diagnostics.is_empty());
        result
    }
}

#[cfg(test)]
pub trait WithDiagnosticAnalysis<T> {
    fn analyze_with_diagnostics(&self) -> (T, Vec<Diagnostic>);
}

#[cfg(test)]
impl<I, T> WithDiagnosticAnalysis<T> for I
where
    I: Analysis<T>,
    T: Debug,
{
    fn analyze_with_diagnostics(&self) -> (T, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let context = AnalysisContext::default();
        let state = RefCell::default();
        let result = self.analyze(&context, &state, &mut diagnostics).unwrap();
        (result, diagnostics)
    }
}
