use crate::dts::analysis::{Analysis, AnalysisContext};
use crate::dts::ast::property::{PropertyList, PropertyValue};
use crate::dts::model::Value;

impl<'a> Analysis<Vec<Value<'a>>> for PropertyList {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<String>,
    ) -> Result<Vec<Value<'a>>, String> {
        todo!()
    }
}

impl<'a> Analysis<Value<'a>> for PropertyValue {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<String>,
    ) -> Result<Value<'a>, String> {
        todo!()
    }
}
