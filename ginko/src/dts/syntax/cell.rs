use crate::dts::expression::token::Token;
use crate::dts::expression::SyntaxKind::*;
use crate::dts::syntax::Parser;

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_cell(&mut self) {
        assert_eq!(self.peek_kind(), Some(L_CHEV));
        self.start_node(CELL);
        self.bump();
        loop {
            if self.peek_kind() == Some(R_CHEV) {
                self.bump();
                break;
            }
            self.parse_cell_content();
        }
        self.finish_node();
    }

    fn parse_cell_content(&mut self) {
        match self.peek_kind() {
            Some(NUMBER) => self.bump_into_node(INT),
            Some(L_PAR) => self.parse_parenthesized_expression(),
            Some(AMP) => self.parse_reference(),
            Some(_) => {
                self.error_token("Expected number, reference or expression");
            }
            _ => self.eof_error(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::syntax::parser::Parser;
    use crate::dts::syntax::testing::check_generic;

    fn check(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_cell)
    }

    #[test]
    fn check_empty_cell() {
        check(
            "<>",
            r#"
CELL
  L_CHEV "<"
  R_CHEV ">"
"#,
        );
    }

    #[test]
    fn check_cell_with_single_element() {
        check(
            "<&some_name>",
            r#"
CELL
  L_CHEV "<"
  REFERENCE
    AMP "&"
    IDENT "some_name"
  R_CHEV ">"
"#,
        );

        check(
            "<5>",
            r#"
CELL
  L_CHEV "<"
  INT
    NUMBER "5"
  R_CHEV ">"
"#,
        );
    }

    #[test]
    fn check_cell_with_homogeneous_elements() {
        check(
            "<8 9>",
            r#"
CELL
  L_CHEV "<"
  INT
    NUMBER "8"
  WHITESPACE " "
  INT
    NUMBER "9"
  R_CHEV ">"
"#,
        );

        check(
            "<&node_a &node_b>",
            r#"
CELL
  L_CHEV "<"
  REFERENCE
    AMP "&"
    IDENT "node_a"
  WHITESPACE " "
  REFERENCE
    AMP "&"
    IDENT "node_b"
  R_CHEV ">"
"#,
        );
    }
    #[test]
    fn check_cell_with_heterogeneous_elements() {
        check(
            "<17 &label>",
            r#"
CELL
  L_CHEV "<"
  INT
    NUMBER "17"
  WHITESPACE " "
  REFERENCE
    AMP "&"
    IDENT "label"
  R_CHEV ">"
"#,
        );
    }
}