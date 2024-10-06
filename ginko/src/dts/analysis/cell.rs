use crate::dts::analysis::{Analysis, AnalysisContext};
use crate::dts::ast::cell::{Cell, CellContent};
use crate::dts::eval::Eval;
use crate::dts::model::CellValue;
use itertools::Itertools;

impl<'a> Analysis<Vec<CellValue<'a>>> for Cell {
    fn analyze(
        &self,
        context: &AnalysisContext,
        diagnostics: &mut Vec<String>,
    ) -> Result<Vec<CellValue<'a>>, String> {
        self.content()
            .map(|content| content.analyze(context, diagnostics))
            .try_collect()
    }
}

impl<'a> Analysis<CellValue<'a>> for CellContent {
    fn analyze(
        &self,
        _context: &AnalysisContext,
        diagnostics: &mut Vec<String>,
    ) -> Result<CellValue<'a>, String> {
        match self {
            CellContent::Number(int) => match int.eval() {
                Ok(res) => Ok(CellValue::U32(res)),
                Err(err) => Err(err.to_string()),
            },
            CellContent::Expression(expr) => match expr.eval() {
                Ok(result) => {
                    let upper_bits = ((result & 0xFFFFFFFF00000000) >> 32) as i32;
                    if upper_bits != 0 && upper_bits != -1 {
                        diagnostics.push("Truncating bits".to_string())
                    }
                    Ok(CellValue::U32((result & 0xFFFFFFFF) as u32))
                }
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
