use crate::dts::ast::expression::{IntConstant, ParenExpression};
use crate::dts::ast::{ast_node, Cast};
use crate::dts::expression::SyntaxKind::*;
use crate::dts::syntax::SyntaxToken;

ast_node! {
    struct Reference(REFERENCE);
}

impl Reference {
    pub fn target(&self) -> String {
        self.0.last_token().unwrap().to_string().to_owned()
    }
}

ast_node! {
    struct CellContent(INT | PAREN_EXPRESSION | REFERENCE);
}

#[derive(Debug)]
pub enum CellContentKind {
    Number(IntConstant),
    Expression(ParenExpression),
    Reference(Reference),
}

impl CellContent {
    pub fn kind(&self) -> CellContentKind {
        match self.0.kind() {
            INT => CellContentKind::Number(IntConstant::cast(self.0.clone()).unwrap()),
            PAREN_EXPRESSION => {
                CellContentKind::Expression(ParenExpression::cast(self.0.clone()).unwrap())
            }
            REFERENCE => CellContentKind::Reference(Reference::cast(self.0.clone()).unwrap()),
            _ => unreachable!(),
        }
    }
}

ast_node! {
    struct Cell(CELL);
}

impl Cell {
    pub fn l_chev(&self) -> SyntaxToken {
        self.0.first_token().unwrap()
    }

    pub fn r_chev(&self) -> SyntaxToken {
        self.0.last_token().unwrap()
    }

    pub fn content(&self) -> impl Iterator<Item = CellContent> {
        self.0.children().filter_map(CellContent::cast)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::cell::{Cell, CellContentKind};
    use crate::dts::ast::Cast;
    use crate::dts::eval::Eval;
    use crate::dts::expression::lex::lex;
    use crate::dts::syntax::Parser;
    use assert_matches::assert_matches;
    use itertools::Itertools;
    use rowan::{TextRange, TextSize};

    fn parse_to_cell(value: &str) -> Cell {
        let (ast, errors) = Parser::new(lex(value).into_iter()).parse(Parser::parse_cell);
        assert!(errors.is_empty(), "Got errors {:?}", errors);
        Cell::cast(ast).unwrap()
    }

    #[test]
    fn check_empty_cell() {
        let cell = parse_to_cell("<>");
        assert_eq!(cell.content().count(), 0);
        assert_eq!(
            cell.l_chev().text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(1))
        );
        assert_eq!(
            cell.r_chev().text_range(),
            TextRange::new(TextSize::new(1), TextSize::new(2))
        );
    }

    #[test]
    fn check_cell_with_single_element() {
        let cell = parse_to_cell("<&some_name>");
        let content = cell.content().collect_vec();
        assert_eq!(content.len(), 1);
        match content[0].kind() {
            CellContentKind::Reference(reference) => {
                assert_eq!(reference.target(), "some_name".to_owned())
            }
            _ => panic!("Expected reference"),
        }
        assert_eq!(
            cell.l_chev().text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(1))
        );
        assert_eq!(
            cell.r_chev().text_range(),
            TextRange::new(TextSize::new(11), TextSize::new(12))
        );
    }

    #[test]
    fn check_cell_with_homogeneous_elements() {
        let cell = parse_to_cell("<8 9>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 2);
        assert!(contents
            .iter()
            .all(|content| matches!(content.kind(), CellContentKind::Number(_))));

        let cell = parse_to_cell("<&node_a &node_b>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 2);
        assert!(contents
            .iter()
            .all(|content| matches!(content.kind(), CellContentKind::Reference(_))));
    }

    #[test]
    fn check_cell_with_heterogeneous_elements() {
        let cell = parse_to_cell("<17 &label>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 2);
        match contents[0].kind() {
            CellContentKind::Number(number) => {
                assert_eq!(number.eval(), Ok(17))
            }
            _ => panic!("Expected number"),
        }
        assert_matches!(contents[1].kind(), CellContentKind::Reference(_));
    }

    #[test]
    fn check_cell_with_expression() {
        let cell = parse_to_cell("<(42 + 69)>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 1);
        match contents[0].kind() {
            CellContentKind::Expression(expr) => {
                assert_eq!(expr.eval(), Ok(111))
            }
            _ => panic!("Expected expression"),
        }
    }
}
