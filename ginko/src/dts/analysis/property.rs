use crate::dts::analysis::{Analysis, AnalysisContext};
use crate::dts::ast::property::{PropertyList, PropertyValue};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::model::Value;

impl<'a> Analysis<Vec<Value<'a>>> for PropertyList {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<Value<'a>>, Diagnostic> {
        todo!()
    }
}

impl<'a> Analysis<Value<'a>> for PropertyValue {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Value<'a>, Diagnostic> {
        todo!()
    }
}
