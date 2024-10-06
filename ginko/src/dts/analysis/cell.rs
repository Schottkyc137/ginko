use crate::dts::analysis::{Analysis, AnalysisContext, BitWidth};
use crate::dts::ast::cell::{Cell, CellContent};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::Eval;
use crate::dts::model::CellValue;
use crate::dts::ErrorCode;
use itertools::Itertools;

impl Analysis<Vec<CellValue>> for Cell {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<Vec<CellValue>, Diagnostic> {
        let bits = match self.bits() {
            None => BitWidth::default(),
            Some(spec) => {
                let res: u32 = spec.bits().eval()?;
                BitWidth::try_from(res).map_err(|_| {
                    Diagnostic::new(
                        spec.bits().range(),
                        ErrorCode::IllegalBitWidth,
                        "Illegal bit width (must be 8, 16, 32 or 64)",
                    )
                })?
            }
        };
        // this stops at the first error.
        // One could alternatively push all errors to the diagnostics; unsure what's better
        self.content()
            .map(|content| content.analyze(&context.with_bit_width(bits), diagnostics))
            .try_collect()
    }
}

impl Analysis<CellValue> for CellContent {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<CellValue, Diagnostic> {
        match self {
            CellContent::Number(int) => match context.bit_width {
                BitWidth::W8 => Ok(CellValue::U8(int.eval()?)),
                BitWidth::W16 => Ok(CellValue::U16(int.eval()?)),
                BitWidth::W32 => Ok(CellValue::U32(int.eval()?)),
                BitWidth::W64 => Ok(CellValue::U64(int.eval()?)),
            },
            CellContent::Expression(expr) => {
                let result = expr.eval()?;
                match context.bit_width {
                    BitWidth::W8 => {
                        let upper_bits = ((result & 0xFFFFFFFFFFFFFF00) >> 8) as i8;
                        if upper_bits != 0 && upper_bits != -1 {
                            diagnostics.push(Diagnostic::new(
                                expr.range(),
                                ErrorCode::IntError,
                                "Truncating bits".to_string(),
                            ))
                        }
                        Ok(CellValue::U8((result & 0xFF) as u8))
                    }
                    BitWidth::W16 => {
                        let upper_bits = ((result & 0xFFFFFFFFFFFF0000) >> 16) as i16;
                        if upper_bits != 0 && upper_bits != -1 {
                            diagnostics.push(Diagnostic::new(
                                expr.range(),
                                ErrorCode::IntError,
                                "Truncating bits".to_string(),
                            ))
                        }
                        Ok(CellValue::U16((result & 0xFFFF) as u16))
                    }
                    BitWidth::W32 => {
                        let upper_bits = ((result & 0xFFFFFFFF00000000) >> 32) as i32;
                        if upper_bits != 0 && upper_bits != -1 {
                            diagnostics.push(Diagnostic::new(
                                expr.range(),
                                ErrorCode::IntError,
                                "Truncating bits".to_string(),
                            ))
                        }
                        Ok(CellValue::U32((result & 0xFFFFFFFF) as u32))
                    }
                    BitWidth::W64 => Ok(CellValue::U64(result)),
                }
            }
            CellContent::Reference(_reference) => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::{ExpectedErrorAnalysis, NoErrorAnalysis};
    use crate::dts::ast::cell::Cell;
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::model::CellValue;
    use crate::dts::ErrorCode;
    use rowan::{TextRange, TextSize};

    #[test]
    fn analyze_simple_cell() {
        let cell = "<32>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(cell, vec![CellValue::U32(32)]);
        let cell = "<(13 + 14)>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(cell, vec![CellValue::U32(27)]);
    }

    #[test]
    fn analyze_cell_with_multiple_values() {
        let cell = "<32 54 0x17>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(
            cell,
            vec![CellValue::U32(32), CellValue::U32(54), CellValue::U32(0x17)]
        );

        let cell = "<(-1) 5>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(cell, vec![CellValue::U32(0xFFFFFFFF), CellValue::U32(5)]);
    }

    #[test]
    fn analyze_cell_with_bits() {
        let cell = "/bits/ 8 <0xFF 0x23>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(cell, vec![CellValue::U8(0xFF), CellValue::U8(0x23)]);

        let cell = "/bits/ 16 <0x241C 0x0809>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(cell, vec![CellValue::U16(0x241C), CellValue::U16(0x0809)]);

        let cell = "/bits/ 32 <0x12345678 0x9ABCDEF0>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(
            cell,
            vec![CellValue::U32(0x12345678), CellValue::U32(0x9ABCDEF0)]
        );

        let cell = "/bits/ 64 <0x0123456789ABCDEF>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(cell, vec![CellValue::U64(0x0123456789ABCDEF)]);
    }

    #[test]
    fn analyze_cell_with_bits_expression() {
        let cell = "/bits/ 64 <(0x0123456789ABCDEF - 33)>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(cell, vec![CellValue::U64(0x123456789ABCDCE)]);
    }

    #[test]
    fn illegal_bit_width_error() {
        let diag = "/bits/ 33 <0xAB>"
            .parse::<Cell>()
            .unwrap()
            .analyze_exp_error();
        assert_eq!(
            diag,
            Diagnostic::new(
                TextRange::new(TextSize::new(7), TextSize::new(9)),
                ErrorCode::IllegalBitWidth,
                "Illegal bit width (must be 8, 16, 32 or 64)"
            )
        );
    }

    #[test]
    fn converting_truncating() {
        let diag = "/bits/ 8 <0xABCD>"
            .parse::<Cell>()
            .unwrap()
            .analyze_exp_error();
        assert_eq!(
            diag,
            Diagnostic::new(
                TextRange::new(TextSize::new(10), TextSize::new(16)),
                ErrorCode::IntError,
                "number too large to fit in target type"
            )
        );

        let diag = "/bits/ 16 <0xABCDEF>"
            .parse::<Cell>()
            .unwrap()
            .analyze_exp_error();
        assert_eq!(
            diag,
            Diagnostic::new(
                TextRange::new(TextSize::new(11), TextSize::new(19)),
                ErrorCode::IntError,
                "number too large to fit in target type"
            )
        );

        let diag = "/bits/ 32 <0xABCDABCDE>"
            .parse::<Cell>()
            .unwrap()
            .analyze_exp_error();
        assert_eq!(
            diag,
            Diagnostic::new(
                TextRange::new(TextSize::new(11), TextSize::new(22)),
                ErrorCode::IntError,
                "number too large to fit in target type"
            )
        );

        let diag = "/bits/ 64 <0xABCDABCDABCDABDCE>"
            .parse::<Cell>()
            .unwrap()
            .analyze_exp_error();
        assert_eq!(
            diag,
            Diagnostic::new(
                TextRange::new(TextSize::new(11), TextSize::new(30)),
                ErrorCode::IntError,
                "number too large to fit in target type"
            )
        );
    }
}
