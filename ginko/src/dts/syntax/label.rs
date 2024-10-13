use crate::dts::lex::token::Token;
use crate::dts::syntax::multipeek::MultiPeek;
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::ErrorCode;
use rowan::TextRange;

impl<M> Parser<M>
where
    M: MultiPeek<Token> + Iterator<Item = Token>,
{
    pub fn parse_optional_label(&mut self) {
        if self.next_kinds_are([IDENT, COLON]) {
            self.start_node(LABEL);
            self.bump_n(2);
            self.finish_node();
        } else if self.next_kinds_are([IDENT, WHITESPACE, COLON]) {
            self.start_node(LABEL);
            self.bump();
            let pos = self.pos();
            self.skip_ws();
            self.push_error(
                TextRange::new(pos, self.pos()),
                ErrorCode::Expected,
                "Whitespace not allowed between identifier and colon",
            );
            self.bump();
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
                TextRange::new(TextSize::new(5), TextSize::new(7)),
                ErrorCode::Expected,
                "Whitespace not allowed between identifier and colon",
            )],
        );
    }
}
