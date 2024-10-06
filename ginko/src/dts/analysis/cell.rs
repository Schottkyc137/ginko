use crate::dts::analysis::{Analysis, AnalysisContext, BitWidth};
use crate::dts::ast::cell::{Cell, CellContent};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::Eval;
use crate::dts::model::{CellValue, CellValues};
use crate::dts::ErrorCode;
use itertools::Itertools;

impl Analysis<CellValues> for Cell {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<CellValues, Diagnostic> {
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
        match bits {
            BitWidth::W8 => self
                .content()
                .map(|content| {
                    <CellContent as Analysis<CellValue<u8>>>::analyze(
                        &content,
                        context,
                        diagnostics,
                    )
                })
                .try_collect(),
            BitWidth::W16 => self
                .content()
                .map(|content| {
                    <CellContent as Analysis<CellValue<u16>>>::analyze(
                        &content,
                        context,
                        diagnostics,
                    )
                })
                .try_collect(),
            BitWidth::W32 => self
                .content()
                .map(|content| {
                    <CellContent as Analysis<CellValue<u32>>>::analyze(
                        &content,
                        context,
                        diagnostics,
                    )
                })
                .try_collect(),
            BitWidth::W64 => self
                .content()
                .map(|content| {
                    <CellContent as Analysis<CellValue<u64>>>::analyze(
                        &content,
                        context,
                        diagnostics,
                    )
                })
                .try_collect(),
        }
    }
}

trait TruncateFrom<T> {
    /// Truncates the value unconditionally.
    /// If the value cannot fill the required space, the returned bool is true.
    fn truncate(value: T) -> (bool, Self);
}

// Truncating from self is always possible and returns self without truncation
impl<I> TruncateFrom<Self> for I {
    fn truncate(value: Self) -> (bool, Self) {
        (false, value)
    }
}

impl TruncateFrom<u64> for u8 {
    fn truncate(value: u64) -> (bool, Self) {
        let test = (value & 0xFFFFFFFFFFFFFF00) >> 8;
        if test != 0xFFFFFFFFFFFFFF && test != 0x00000000000000 {
            (true, (value & 0xFF) as u8)
        } else {
            (false, (value & 0xFF) as u8)
        }
    }
}

impl TruncateFrom<u64> for u16 {
    fn truncate(value: u64) -> (bool, Self) {
        let test = (value & 0xFFFFFFFFFFFF0000) >> 16;
        if test != 0xFFFFFFFFFFFF && test != 0x000000000000 {
            (true, (value & 0xFFFF) as u16)
        } else {
            (false, (value & 0xFFFF) as u16)
        }
    }
}

impl TruncateFrom<u64> for u32 {
    fn truncate(value: u64) -> (bool, Self) {
        let test = (value & 0xFFFFFFFF00000000) >> 32;
        if test != 0xFFFFFFFF && test != 0x00000000 {
            (true, (value & 0xFFFFFFFF) as u32)
        } else {
            (false, (value & 0xFFFFFFFF) as u32)
        }
    }
}

macro_rules! analysis_from_int {
    ($($t:ident),+) => {
        $(
            impl Analysis<CellValue<$t>> for CellContent {
                fn analyze(
                    &self,
                    _context: &AnalysisContext,
                    diagnostics: &mut Vec<Diagnostic>,
                ) -> Result<CellValue<$t>, Diagnostic> {
                    match self {
                        CellContent::Number(int) => Ok(CellValue::Number(int.eval()?)),
                        CellContent::Expression(expr) => {
                            let result = expr.eval()?;
                            let (has_truncated, truncated) = $t::truncate(result);
                            if has_truncated {
                                diagnostics.push(Diagnostic::new(
                                    expr.range(),
                                    ErrorCode::IntError,
                                    "Truncating bits".to_string(),
                                ))
                            }
                            Ok(CellValue::Number(truncated))
                        }
                        CellContent::Reference(_reference) => unimplemented!(),
                    }
                }
            }
        )+
    };
}

analysis_from_int!(u8, u16, u32, u64);

#[cfg(test)]
mod tests {
    use crate::dts::analysis::{ExpectedErrorAnalysis, NoErrorAnalysis};
    use crate::dts::ast::cell::Cell;
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::model::{CellValue, CellValues};
    use crate::dts::ErrorCode;
    use rowan::{TextRange, TextSize};

    #[test]
    fn analyze_simple_cell() {
        let cell = "<32>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(cell, CellValues::U32(vec![CellValue::Number(32)]));
        let cell = "<(13 + 14)>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(cell, CellValues::U32(vec![CellValue::Number(27)]));
    }

    #[test]
    fn analyze_cell_with_multiple_values() {
        let cell = "<32 54 0x17>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(
            cell,
            CellValues::U32(vec![
                CellValue::Number(32),
                CellValue::Number(54),
                CellValue::Number(0x17)
            ])
        );

        let cell = "<(-1) 5>".parse::<Cell>().unwrap().analyze_no_errors();
        assert_eq!(
            cell,
            CellValues::U32(vec![CellValue::Number(0xFFFFFFFF), CellValue::Number(5)])
        );
    }

    #[test]
    fn analyze_cell_with_bits() {
        let cell = "/bits/ 8 <0xFF 0x23>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(
            cell,
            CellValues::U8(vec![CellValue::Number(0xFF), CellValue::Number(0x23)])
        );

        let cell = "/bits/ 16 <0x241C 0x0809>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(
            cell,
            CellValues::U16(vec![CellValue::Number(0x241C), CellValue::Number(0x0809)])
        );

        let cell = "/bits/ 32 <0x12345678 0x9ABCDEF0>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(
            cell,
            CellValues::U32(vec![
                CellValue::Number(0x12345678),
                CellValue::Number(0x9ABCDEF0)
            ])
        );

        let cell = "/bits/ 64 <0x0123456789ABCDEF>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(
            cell,
            CellValues::U64(vec![CellValue::Number(0x0123456789ABCDEF)])
        );
    }

    #[test]
    fn analyze_cell_with_bits_expression() {
        let cell = "/bits/ 64 <(0x0123456789ABCDEF - 33)>"
            .parse::<Cell>()
            .unwrap()
            .analyze_no_errors();
        assert_eq!(
            cell,
            CellValues::U64(vec![CellValue::Number(0x123456789ABCDCE)])
        );
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
