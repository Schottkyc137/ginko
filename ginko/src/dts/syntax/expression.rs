use crate::dts::lex::token::Token;
use crate::dts::syntax::parser::Parser;
use crate::dts::syntax::SyntaxKind;
use crate::dts::syntax::SyntaxKind::*;
use rowan::Checkpoint;

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_expression(&mut self) {
        let lhs = self.checkpoint();
        self.parse_unary();
        self._parse_expression(lhs, 0);
    }

    fn _parse_expression(&mut self, lhs: Checkpoint, min_precedence: usize) {
        while let Some(precedence) = self.peek_kind().and_then(binary_precedence) {
            if precedence < min_precedence {
                break;
            }
            self.start_node_at(lhs, BINARY);
            self.bump_into_node(OP);
            let rhs = self.checkpoint();
            self.parse_unary();
            while self
                .peek_kind()
                .and_then(binary_precedence)
                .is_some_and(|inner_precedence| inner_precedence > precedence)
            {
                self._parse_expression(rhs, precedence + 1);
            }
            self.finish_node();
        }
    }

    pub fn parse_parenthesized_expression(&mut self) {
        assert_eq!(self.peek_kind(), Some(L_PAR));
        self.start_node(PAREN_EXPRESSION);
        self.bump();
        self.parse_expression();
        self.expect(R_PAR);
        self.finish_node();
    }

    pub fn parse_primary(&mut self) {
        match self.peek_kind() {
            Some(L_PAR) => {
                self.parse_parenthesized_expression();
            }
            Some(NUMBER) => {
                self.start_node(INT);
                self.bump();
                self.finish_node()
            }
            _ => {
                self.error_token("Expecting number or '('".to_string());
            }
        }
    }

    pub fn parse_unary(&mut self) {
        let mut unary_count = 0;
        while matches!(self.peek_kind(), Some(TILDE | EXCLAMATION | MINUS)) {
            self.start_node(UNARY);
            self.bump_into_node(OP);
            unary_count += 1;
        }
        self.parse_primary();
        for _ in 0..unary_count {
            self.finish_node()
        }
    }
}

fn binary_precedence(kind: SyntaxKind) -> Option<usize> {
    Some(match kind {
        STAR | SLASH | PERCENT => 11,
        MINUS | PLUS => 10,
        DOUBLE_R_CHEV | DOUBLE_L_CHEV => 9,
        R_CHEV | L_CHEV | LTE | GTE => 8,
        EQEQ | NEQ => 7,
        AMP => 6,
        CIRC => 5,
        BAR => 4,
        DOUBLE_AMP => 3,
        DOUBLE_BAR => 2,
        QUESTION_MARK => 1,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::dts::syntax::parser::Parser;
    use crate::dts::syntax::testing::check_generic;

    fn check(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_expression)
    }

    #[test]
    fn check_primary() {
        check(
            "1",
            r#"
INT
  NUMBER "1"
"#,
        );
    }

    #[test]
    fn check_inner_expression() {
        check(
            "1+(2)",
            r#"
BINARY
  INT
    NUMBER "1"
  OP
    PLUS "+"
  PAREN_EXPRESSION
    L_PAR "("
    INT
      NUMBER "2"
    R_PAR ")"
"#,
        );
    }

    #[test]
    fn check_simple_binary() {
        check(
            "1+2",
            r#"
BINARY
  INT
    NUMBER "1"
  OP
    PLUS "+"
  INT
    NUMBER "2"
"#,
        );
    }

    #[test]
    fn check_nested_binary() {
        check(
            "1+2*3",
            r#"
BINARY
  INT
    NUMBER "1"
  OP
    PLUS "+"
  BINARY
    INT
      NUMBER "2"
    OP
      STAR "*"
    INT
      NUMBER "3"
"#,
        );
        check(
            "2+3*4+5",
            r#"
BINARY
  BINARY
    INT
      NUMBER "2"
    OP
      PLUS "+"
    BINARY
      INT
        NUMBER "3"
      OP
        STAR "*"
      INT
        NUMBER "4"
  OP
    PLUS "+"
  INT
    NUMBER "5"
"#,
        );
    }

    #[test]
    fn check_simple_unary() {
        check(
            "!1",
            r#"
UNARY
  OP
    EXCLAMATION "!"
  INT
    NUMBER "1"
"#,
        );
    }

    #[test]
    fn check_double_unary() {
        check(
            "~!1",
            r#"
UNARY
  OP
    TILDE "~"
  UNARY
    OP
      EXCLAMATION "!"
    INT
      NUMBER "1"
"#,
        );
    }
}
