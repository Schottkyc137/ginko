use crate::dts::analysis::{Analysis, AnalysisContext, ProjectState, PushIntoDiagnostics};
use crate::dts::ast::property::{PropertyList, PropertyValue, PropertyValueKind};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::{Eval, InfallibleEval};
use crate::dts::model::Value;
use itertools::Itertools;
use std::cell::RefCell;

impl Analysis<Vec<Value>> for PropertyList {
    fn analyze(
        &self,
        context: &AnalysisContext,
        project: &RefCell<ProjectState>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<Value>, Diagnostic> {
        Ok(self
            .items()
            .filter_map(|item| {
                item.analyze(context, project, diagnostics)
                    .or_push_into(diagnostics)
            })
            .collect_vec())
    }
}

impl Analysis<Value> for PropertyValue {
    fn analyze(
        &self,
        context: &AnalysisContext,
        project: &RefCell<ProjectState>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Value, Diagnostic> {
        match self.kind() {
            PropertyValueKind::String(string) => Ok(Value::String(string.value())),
            PropertyValueKind::Cell(cell) => {
                Ok(Value::Cell(cell.analyze(context, project, diagnostics)?))
            }
            PropertyValueKind::Reference(_reference) => unimplemented!(),
            PropertyValueKind::ByteString(byte_string) => Ok(Value::Bytes(byte_string.eval()?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::{NoErrorAnalysis, WithDiagnosticAnalysis};
    use crate::dts::ast::property::{PropertyList, PropertyValue};
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::model::{CellValue, CellValues, Value};
    use crate::dts::ErrorCode;
    use rowan::{TextRange, TextSize};

    #[test]
    fn eval_simple_property_value() {
        let str = r#""Hello, World!""#.parse::<PropertyValue>().unwrap().analyze_no_errors();
        assert_eq!(str, Value::String("Hello, World!".to_owned()));
        let cell = "<17>".parse::<PropertyValue>().unwrap().analyze_no_errors();
        assert_eq!(
            cell,
            Value::Cell(CellValues::U32(vec![CellValue::Number(17)]))
        );
        let bytes = "[ABCD EF 01]"
            .parse::<PropertyValue>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(bytes, Value::Bytes(vec![0xAB, 0xCD, 0xEF, 0x01]));
    }

    #[test]
    fn eval_property_list() {
        let properties =
            r#""Hello, World!", <17>"#.parse::<PropertyList>().unwrap().analyze_no_errors();
        assert_eq!(
            properties,
            vec![
                Value::String("Hello, World!".into()),
                Value::Cell(CellValues::U32(vec![CellValue::Number(17)]))
            ]
        )
    }

    #[test]
    fn property_list_continues_after_error() {
        let (properties, diagnostics) = r#""Hello, World!", /bits/ 8 <0xABCD>"#
            .parse::<PropertyList>()
            .unwrap()
            .analyze_with_diagnostics();
        assert_eq!(properties, vec![Value::String("Hello, World!".into()),]);
        assert_eq!(
            diagnostics,
            vec![Diagnostic::new(
                TextRange::new(TextSize::new(27), TextSize::new(33)),
                ErrorCode::IntError,
                "number too large to fit in target type"
            )]
        )
    }
}
