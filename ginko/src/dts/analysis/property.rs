use crate::dts::analysis::{Analyzer, PushIntoDiagnostics};
use crate::dts::ast::cell::Reference;
use crate::dts::ast::property::{PropertyList, PropertyValue, PropertyValueKind};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::{Eval, InfallibleEval};
use crate::dts::model;
use crate::dts::model::Value;
use itertools::Itertools;

impl Analyzer {
    pub fn analyze_property_list(
        &self,
        property_list: &PropertyList,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<Value> {
        property_list
            .items()
            .filter_map(|item| {
                self.analyze_property_value(&item, diagnostics)
                    .or_push_into(diagnostics)
            })
            .collect_vec()
    }

    pub fn analyze_property_value(
        &self,
        value: &PropertyValue,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Value, Diagnostic> {
        match value.kind() {
            PropertyValueKind::String(string) => Ok(Value::String(string.value())),
            PropertyValueKind::Cell(cell) => {
                Ok(Value::Cell(self.analyze_cell(&cell, diagnostics)?))
            }
            PropertyValueKind::Reference(reference) => match reference {
                Reference::Ref(reference) => Ok(Value::Reference(model::Reference::Label(
                    reference.target(),
                ))),
                Reference::RefPath(path) => Ok(Value::Reference(model::Reference::Path(
                    path.target().eval()?,
                ))),
            },
            PropertyValueKind::ByteString(byte_string) => Ok(Value::Bytes(byte_string.eval()?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::{
        Analyzer, NoErrorAnalysis, PushIntoDiagnostics, WithDiagnosticAnalysis,
    };
    use crate::dts::ast::property::{PropertyList, PropertyValue};
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::model::{CellValue, CellValues, Value};
    use crate::dts::ErrorCode;
    use rowan::{TextRange, TextSize};

    impl WithDiagnosticAnalysis<Value> for PropertyValue {
        fn analyze_with_diagnostics(&self) -> (Option<Value>, Vec<Diagnostic>) {
            let analyzer = Analyzer::new();
            let mut diagnostics = Vec::new();
            let value = analyzer
                .analyze_property_value(self, &mut diagnostics)
                .or_push_into(&mut diagnostics);
            (value, diagnostics)
        }
    }

    impl WithDiagnosticAnalysis<Vec<Value>> for PropertyList {
        fn analyze_with_diagnostics(&self) -> (Option<Vec<Value>>, Vec<Diagnostic>) {
            let analyzer = Analyzer::new();
            let mut diagnostics = Vec::new();
            let value = analyzer.analyze_property_list(self, &mut diagnostics);
            (Some(value), diagnostics)
        }
    }

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
        assert_eq!(
            properties,
            Some(vec![Value::String("Hello, World!".into())])
        );
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
