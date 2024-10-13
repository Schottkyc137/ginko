use crate::dts::lex::token::Token;
use crate::dts::syntax::multipeek::MultiPeek;
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;

impl<M> Parser<M>
where
    M: MultiPeek<Token> + Iterator<Item = Token>,
{
    fn parse_bits_directive(&mut self) {
        self.start_node(BITS_SPEC);
        self.bump();
        match self.peek_kind() {
            Some(NUMBER) => self.bump_into_node(INT),
            Some(L_CHEV) => self.error_node("Expected number of bits"),
            Some(_) => self.error_token("Expected number of bits"),
            None => self.unexpected_eof(),
        }
        self.finish_node();
    }

    pub fn parse_cell(&mut self) {
        self.start_node(CELL);
        if self.peek_kind() == Some(BITS) {
            self.parse_bits_directive();
        }
        self.skip_ws();
        self.start_node(CELL_INNER);
        self.expect(L_CHEV);
        loop {
            match self.peek_kind() {
                None => {
                    self.unexpected_eof();
                    break;
                }
                Some(R_CHEV) => {
                    self.bump();
                    break;
                }
                Some(_) => self.parse_cell_content(),
            }
        }
        self.finish_node();
        self.finish_node();
    }

    fn parse_cell_content(&mut self) {
        self.parse_optional_label();
        match self.peek_kind() {
            Some(NUMBER) => self.bump_into_node(INT),
            Some(L_PAR) => self.parse_parenthesized_expression(),
            Some(AMP) => self.parse_reference(),
            Some(_) => {
                self.error_token("Expected number, reference or expression");
            }
            None => {}
        }
        self.parse_optional_label();
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::syntax::parser::Parser;
    use crate::dts::syntax::testing::{check_generic, check_generic_diag};
    use crate::dts::ErrorCode;
    use rowan::{TextRange, TextSize};

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
    REF
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
    REF
      AMP "&"
      IDENT "node_a"
    WHITESPACE " "
    REF
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
    REF
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

    #[test]
    fn check_error_after_bits() {
        check_generic_diag(
            &[Diagnostic::new(
                TextRange::new(TextSize::new(7), TextSize::new(7)),
                ErrorCode::Expected,
                "Expected number of bits",
            )],
            "/bits/ <0xABCD>",
            r#"
CELL
  BITS_SPEC
    BITS "/bits/"
    WHITESPACE " "
  CELL_INNER
    L_CHEV "<"
    INT
      NUMBER "0xABCD"
    R_CHEV ">"
"#,
            Parser::parse_cell,
        );
        check_generic_diag(
            &[Diagnostic::new(
                TextRange::new(TextSize::new(7), TextSize::new(12)),
                ErrorCode::Expected,
                "Expected number of bits",
            )],
            "/bits/ eight <0xABCD>",
            r#"
CELL
  BITS_SPEC
    BITS "/bits/"
    WHITESPACE " "
    ERROR
      IDENT "eight"
  WHITESPACE " "
  CELL_INNER
    L_CHEV "<"
    INT
      NUMBER "0xABCD"
    R_CHEV ">"
"#,
            Parser::parse_cell,
        );
    }

    #[test]
    fn check_eof_after_bits() {
        check_generic_diag(
            &[Diagnostic::new(
                TextRange::new(TextSize::new(6), TextSize::new(6)),
                ErrorCode::UnexpectedEOF,
                "Unexpected EOF",
            )],
            "/bits/",
            r#"
CELL
  BITS_SPEC
    BITS "/bits/"
  CELL_INNER
"#,
            Parser::parse_cell,
        );
    }

    #[test]
    fn optional_label_contents() {
        check(
            "<label: 5>",
            r#"
CELL
  CELL_INNER
    L_CHEV "<"
    LABEL
      IDENT "label"
      COLON ":"
    WHITESPACE " "
    INT
      NUMBER "5"
    R_CHEV ">"
"#,
        );

        check(
            "<leading: 5 trailing:>",
            r#"
CELL
  CELL_INNER
    L_CHEV "<"
    LABEL
      IDENT "leading"
      COLON ":"
    WHITESPACE " "
    INT
      NUMBER "5"
    WHITESPACE " "
    LABEL
      IDENT "trailing"
      COLON ":"
    R_CHEV ">"
"#,
        );
    }
}
