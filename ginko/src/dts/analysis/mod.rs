use crate::dts::diagnostics::Diagnostic;
use crate::dts::FileType;
use std::fmt::Debug;

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
impl<I, T> NoErrorAnalysis<T> for I
where
    I: Analysis<T>,
{
    fn analyze_no_errors(&self) -> T {
        let mut diagnostics = Vec::new();
        let context = AnalysisContext::default();
        let result = self.analyze(&context, &mut diagnostics).unwrap();
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
        let result = self.analyze(&context, &mut diagnostics).unwrap_err();
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
        let result = self.analyze(&context, &mut diagnostics).unwrap();
        (result, diagnostics)
    }
}
