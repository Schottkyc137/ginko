use crate::dts::lex::token::Token;
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::ErrorCode;
use rowan::TextRange;

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_reference(&mut self) {
        assert_eq!(self.peek_kind(), Some(AMP));
        let checkpoint = self.checkpoint();
        self.bump();
        if self.peek_kind_direct() == Some(WHITESPACE) {
            let pos = self.pos();
            self.skip_ws();
            if matches!(self.peek_kind_direct(), Some(IDENT | L_BRACE)) {
                self.diagnostic(
                    TextRange::new(pos, self.pos()),
                    ErrorCode::Expected,
                    "No whitespace allowed",
                )
            }
        }
        match self.peek_kind_direct() {
            Some(IDENT) => {
                self.start_node_at(checkpoint, REFERENCE);
                self.bump();
            }
            Some(L_BRACE) => {
                self.start_node_at(checkpoint, REF_PATH);
                self.bump();
                self.parse_path();
                self.expect(R_BRACE);
            }
            // do not consume for these kinds
            Some(R_BRAK | SEMICOLON | COMMA) => self.error_node("Expected reference or path"),
            Some(_) => {
                self.error_token("Expected reference or path");
            }
            None => {
                self.eof_error();
                return;
            }
        }
        self.finish_node();
    }

    pub fn parse_path(&mut self) {
        self.start_node(PATH);
        if self.peek_kind_direct() == Some(IDENT) {
            self.error_node("Paths must begin with a slash");
            self.bump();
        }
        while self.peek_kind_direct() == Some(SLASH) {
            self.bump();
            self.node_name();
        }
        self.finish_node();
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::syntax::testing::{check_generic, check_generic_diag};
    use crate::dts::syntax::Parser;
    use crate::dts::ErrorCode;
    use rowan::{TextRange, TextSize};

    fn check_reference(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_reference)
    }

    fn check_reference_diagnostic(expression: &str, expected: &str, diag: &[Diagnostic]) {
        check_generic_diag(diag, expression, expected, Parser::parse_reference)
    }

    #[test]
    fn check_simple_reference() {
        check_reference(
            "&some_label",
            r#"
REFERENCE
  AMP "&"
  IDENT "some_label"
"#,
        );
    }

    #[test]
    fn check_reference_space() {
        check_reference_diagnostic(
            "&  some_label",
            r#"
REFERENCE
  AMP "&"
  WHITESPACE "  "
  IDENT "some_label"
"#,
            &[Diagnostic::new(
                TextRange::new(TextSize::new(1), TextSize::new(3)),
                ErrorCode::Expected,
                "No whitespace allowed",
            )],
        );
    }

    #[test]
    fn check_reference_path() {
        check_reference(
            "&{/path/to/node}",
            r#"
REF_PATH
  AMP "&"
  L_BRACE "{"
  PATH
    SLASH "/"
    NAME
      IDENT "path"
    SLASH "/"
    NAME
      IDENT "to"
    SLASH "/"
    NAME
      IDENT "node"
  R_BRACE "}"
"#,
        )
    }
}
