use crate::dts::analysis::{Analysis, AnalysisContext, BitWidth};
use crate::dts::ast::cell::{Cell, CellContent};
use crate::dts::ast::expression::IntConstant;
use crate::dts::eval::Eval;
use crate::dts::model::CellValue;
use itertools::Itertools;

impl<'a> Analysis<Vec<CellValue<'a>>> for Cell {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<String>,
    ) -> Result<Vec<CellValue<'a>>, String> {
        let bits = match self.bits() {
            None => BitWidth::default(),
            Some(spec) => match <IntConstant as Eval<u32, _>>::eval(&spec.bits()) {
                Ok(res) => match BitWidth::try_from(res) {
                    Ok(res) => res,
                    Err(_) => return Err("Illegal bit width (must be 8, 16, 32 or 64)".to_string()),
                },
                Err(e) => return Err(e.to_string()),
            },
        };
        self.content()
            .map(|content| content.analyze(&context.with_bit_width(bits), diagnostics))
            .try_collect()
    }
}

impl<'a> Analysis<CellValue<'a>> for CellContent {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<String>,
    ) -> Result<CellValue<'a>, String> {
        match self {
            CellContent::Number(int) => match context.bit_width {
                BitWidth::W8 => match int.eval() {
                    Ok(res) => Ok(CellValue::U8(res)),
                    Err(err) => Err(err.to_string()),
                },
                BitWidth::W16 => match int.eval() {
                    Ok(res) => Ok(CellValue::U16(res)),
                    Err(err) => Err(err.to_string()),
                },
                BitWidth::W32 => match int.eval() {
                    Ok(res) => Ok(CellValue::U32(res)),
                    Err(err) => Err(err.to_string()),
                },
                BitWidth::W64 => match int.eval() {
                    Ok(res) => Ok(CellValue::U64(res)),
                    Err(err) => Err(err.to_string()),
                },
            },
            CellContent::Expression(expr) => match expr.eval() {
                Ok(result) => match context.bit_width {
                    BitWidth::W8 => {
                        let upper_bits = ((result & 0xFFFFFFFFFFFFFF00) >> 8) as i8;
                        if upper_bits != 0 && upper_bits != -1 {
                            diagnostics.push("Truncating bits".to_string())
                        }
                        Ok(CellValue::U8((result & 0xFF) as u8))
                    }
                    BitWidth::W16 => {
                        let upper_bits = ((result & 0xFFFFFFFFFFFF0000) >> 16) as i16;
                        if upper_bits != 0 && upper_bits != -1 {
                            diagnostics.push("Truncating bits".to_string())
                        }
                        Ok(CellValue::U16((result & 0xFFFF) as u16))
                    }
                    BitWidth::W32 => {
                        let upper_bits = ((result & 0xFFFFFFFF00000000) >> 32) as i32;
                        if upper_bits != 0 && upper_bits != -1 {
                            diagnostics.push("Truncating bits".to_string())
                        }
                        Ok(CellValue::U32((result & 0xFFFFFFFF) as u32))
                    }
                    BitWidth::W64 => Ok(CellValue::U64(result)),
                },
                Err(err) => Err(err.to_string()),
            },
            CellContent::Reference(_reference) => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::NoErrorAnalysis;
    use crate::dts::ast::cell::Cell;
    use crate::dts::model::CellValue;

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
}
