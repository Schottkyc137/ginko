use crate::dts::analysis::bit_width::BitWidth;
use crate::dts::analysis::cyclic_dependency::CyclicDependencyEntry;
use crate::dts::analysis::project::ProjectState;
use crate::dts::diagnostics::Diagnostic;
use crate::dts::FileType;
use std::path::PathBuf;

mod bit_width;
mod cell;
mod cyclic_dependency;
mod file;
mod node;
pub mod project;
mod property;

#[derive(Default)]
pub struct Analyzer {}

impl Analyzer {
    pub fn new() -> Analyzer {
        Self::default()
    }
}

#[derive(Default, Clone)]
pub struct AnalysisContext {
    pub bit_width: BitWidth,
    pub file_type: FileType,
    pub current: PathBuf,
    pub path: Vec<CyclicDependencyEntry>,
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
        project: &ProjectState,
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

#[cfg(test)]
pub trait NoErrorAnalysis<T> {
    fn analyze_no_errors(&self) -> T;
}

#[cfg(test)]
pub trait WithDiagnosticAnalysis<T> {
    fn analyze_with_diagnostics(&self) -> (Option<T>, Vec<Diagnostic>);

    fn analyze_exp_error(&self) -> Vec<Diagnostic> {
        let (_, diag) = self.analyze_with_diagnostics();
        diag
    }
}

#[cfg(test)]
impl<E, T> NoErrorAnalysis<T> for E
where
    E: WithDiagnosticAnalysis<T>,
{
    fn analyze_no_errors(&self) -> T {
        let (res, diagnostics) = self.analyze_with_diagnostics();
        assert!(
            diagnostics.is_empty(),
            "Unexpectedly got diagnostics: {:?}",
            diagnostics
        );
        res.unwrap()
    }
}
