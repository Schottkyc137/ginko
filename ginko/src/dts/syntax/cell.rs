use crate::dts::lex::token::Token;
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_cell(&mut self) {
        self.start_node(CELL);
        if self.peek_kind() == Some(BITS) {
            self.start_node(BITS_SPEC);
            self.bump();
            match self.peek_kind() {
                Some(NUMBER) => self.bump_into_node(INT),
                Some(_) => self.error_token("Expected number of bits"),
                None => {
                    self.eof_error();
                    self.finish_node();
                    self.finish_node();
                    return;
                }
            }
            self.finish_node();
        }
        self.skip_ws();
        self.start_node(CELL_INNER);
        assert_eq!(self.peek_kind(), Some(L_CHEV));
        self.bump();
        loop {
            if self.peek_kind() == Some(R_CHEV) {
                self.bump();
                break;
            }
            self.parse_cell_content();
        }
        self.finish_node();
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
  CELL_INNER
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
  CELL_INNER
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
  CELL_INNER
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
  CELL_INNER
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
  CELL_INNER
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
  CELL_INNER
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

    #[test]
    fn check_cell_with_bits() {
        check(
            "/bits/ 16 <0xABCD>",
            r#"
CELL
  BITS_SPEC
    BITS "/bits/"
    WHITESPACE " "
    INT
      NUMBER "16"
  WHITESPACE " "
  CELL_INNER
    L_CHEV "<"
    INT
      NUMBER "0xABCD"
    R_CHEV ">"
"#,
        );
    }
}
