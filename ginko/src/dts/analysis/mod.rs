mod cell;
mod property;

pub struct AnalysisContext {}

pub trait Analysis<T> {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<String>,
    ) -> Result<T, String>;
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
        let context = AnalysisContext {};
        let result = self.analyze(&context, &mut diagnostics).unwrap();
        assert!(diagnostics.is_empty());
        result
    }
}
