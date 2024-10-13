use crate::dts::lex::token::Token;
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::ErrorCode;
use rowan::TextRange;

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_optional_label(&mut self) {
        if self.peek_kind() == Some(IDENT) {
            self.start_node(LABEL);
            self.bump();
            match self.peek_kind_direct() {
                Some(COLON) => {
                    self.bump();
                }
                Some(WHITESPACE) => {
                    let pos = self.pos();
                    self.skip_ws();
                    if self.peek_kind_direct() == Some(COLON) {
                        self.bump();
                        self.push_error(
                            TextRange::new(pos, self.pos()),
                            ErrorCode::Expected,
                            "Whitespace not allowed between identifier and colon",
                        );
                    } else {
                        self.push_error(TextRange::empty(pos), ErrorCode::Expected, "Expected ':'")
                    }
                }
                Some(_) => self.expect(COLON),
                None => {
                    self.unexpected_eof();
                }
            }
            self.finish_node();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::syntax::testing::{check_generic, check_generic_diag};
    use crate::dts::syntax::Parser;
    use crate::dts::ErrorCode;
    use rowan::{TextRange, TextSize};

    fn check(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_optional_label)
    }

    fn check_diag(expression: &str, expected: &str, diag: &[Diagnostic]) {
        check_generic_diag(diag, expression, expected, Parser::parse_optional_label)
    }

    #[test]
    fn parse_label() {
        check(
            "label:",
            r#"
LABEL
  IDENT "label"
  COLON ":"
"#,
        );
    }

    #[test]
    fn parse_label_whitespace() {
        check_diag(
            "label  :",
            r#"
LABEL
  IDENT "label"
  WHITESPACE "  "
  COLON ":"
"#,
            &[Diagnostic::new(
                TextRange::new(TextSize::new(5), TextSize::new(8)),
                ErrorCode::Expected,
                "Whitespace not allowed between identifier and colon",
            )],
        );
    }

    #[test]
    fn parse_label_no_colon() {
        check_diag(
            "label",
            r#"
LABEL
  IDENT "label"
"#,
            &[Diagnostic::new(
                TextRange::new(TextSize::new(5), TextSize::new(5)),
                ErrorCode::UnexpectedEOF,
                "Unexpected EOF",
            )],
        );
    }
}
