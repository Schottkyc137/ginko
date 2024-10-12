use crate::dts::diagnostics::Diagnostic;
use crate::dts::lex::token::Token;
use crate::dts::syntax::SyntaxKind;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::SyntaxNode;
use crate::dts::ErrorCode;
use itertools::Itertools;
use rowan::{Checkpoint, GreenNodeBuilder, TextLen, TextRange, TextSize};
use std::iter::Peekable;

pub struct Parser<I: Iterator<Item = Token>> {
    builder: GreenNodeBuilder<'static>,
    iter: Peekable<I>,
    non_ws_pos: TextSize,
    pos: TextSize,
    errors: Vec<Diagnostic>,
    // This is to avoid multiple EOF errors
    unexpected_eof: bool,
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn new(iter: I) -> Parser<I> {
        Parser {
            builder: GreenNodeBuilder::new(),
            iter: iter.peekable(),
            errors: Vec::new(),
            non_ws_pos: TextSize::default(),
            pos: TextSize::default(),
            unexpected_eof: false,
        }
    }
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub(crate) fn skip_ws(&mut self) {
        while self
            .iter
            .peek()
            .map(|token| token.is_whitespace())
            .unwrap_or(false)
        {
            self.bump();
        }
    }

    pub fn pos(&self) -> TextSize {
        self.pos
    }

    pub fn diagnostic(&mut self, range: TextRange, code: ErrorCode, message: impl Into<String>) {
        self.errors.push(Diagnostic::new(range, code, message))
    }

    pub(crate) fn peek_kind(&mut self) -> Option<SyntaxKind> {
        self.skip_ws();
        self.iter.peek().map(|token| token.kind)
    }

    pub(crate) fn peek_kind_direct(&mut self) -> Option<SyntaxKind> {
        self.peek_direct().map(|token| token.kind)
    }

    pub(crate) fn peek_direct(&mut self) -> Option<&Token> {
        self.iter.peek()
    }

    fn expect_error(&mut self, kinds: &[SyntaxKind]) {
        self.error_token(format!(
            "Expecting {}",
            kinds.iter().map(|kind| kind.to_string()).join(", ")
        ))
    }

    pub(crate) fn expect(&mut self, kind: SyntaxKind) {
        if self.peek_kind().is_some_and(|peeked| peeked == kind) {
            self.bump();
        } else {
            let range = match kind {
                SEMICOLON => {
                    if self.peek_kind() == Some(COLON) {
                        self.bump().unwrap()
                    } else {
                        TextRange::empty(self.non_ws_pos)
                    }
                }
                _ => self.bump().unwrap(),
            };
            self.errors.push(Diagnostic::new(
                range,
                ErrorCode::Expected,
                format!("Expecting {}", kind),
            ));
        }
    }

    pub(crate) fn expect_direct(&mut self, kind: SyntaxKind) {
        if self.peek_kind_direct().is_some_and(|peeked| kind == peeked) {
            self.bump();
        } else {
            self.expect_error(&[kind]);
        }
    }

    pub(crate) fn bump(&mut self) -> Option<TextRange> {
        let curr_pos = self.pos;
        if let Some(token) = self.iter.next() {
            self.pos += token.value.text_len();
            if token.kind != WHITESPACE {
                self.non_ws_pos = self.pos;
            }
            self.builder.token(token.kind.into(), token.value.as_str());
            Some(TextRange::new(curr_pos, self.pos))
        } else {
            None
        }
    }

    pub(crate) fn checkpoint(&self) -> Checkpoint {
        self.builder.checkpoint()
    }

    pub(crate) fn start_node(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind.into())
    }

    pub(crate) fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind) {
        self.builder.start_node_at(checkpoint, kind.into())
    }

    pub(crate) fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    pub(crate) fn bump_into_node(&mut self, node: SyntaxKind) {
        self.start_node(node);
        self.bump();
        self.finish_node();
    }

    pub(crate) fn error_token(&mut self, message: impl Into<String>) {
        self.start_node(ERROR);
        let range = self.bump().unwrap();
        self.finish_node();
        self.errors
            .push(Diagnostic::new(range, ErrorCode::Expected, message));
    }

    pub(crate) fn eof_error(&mut self) {
        self.errors.push(Diagnostic::new(
            TextRange::empty(self.pos),
            ErrorCode::UnexpectedEOF,
            "Unexpected EOF",
        ));
        self.start_node(ERROR);
        self.finish_node();
    }

    pub(crate) fn error_node(&mut self, message: impl Into<String>) {
        // There is a previous EOF
        if self.unexpected_eof {
            return;
        }
        self.unexpected_eof = true;
        self.start_node(ERROR);
        self.errors.push(Diagnostic::new(
            TextRange::empty(self.pos),
            ErrorCode::Expected,
            message,
        ));
        self.finish_node();
    }
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse(mut self, target: impl FnOnce(&mut Parser<I>)) -> (SyntaxNode, Vec<Diagnostic>) {
        target(&mut self);
        let node = self.builder.finish();
        let root = SyntaxNode::new_root(node);
        (root, self.errors)
    }
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_mem_reserve(&mut self) {
        assert_eq!(self.peek_kind(), Some(MEM_RESERVE));
        self.start_node(RESERVE_MEMORY);
        self.bump();
        match self.peek_kind() {
            Some(NUMBER) => self.bump_into_node(INT),
            Some(_) => self.error_token("Expected address"),
            None => {
                self.eof_error();
                self.finish_node();
                return;
            }
        }
        match self.peek_kind() {
            Some(NUMBER) => self.bump_into_node(INT),
            Some(_) => self.error_token("Expected length"),
            None => {
                self.eof_error();
                self.finish_node();
                return;
            }
        }
        self.expect(SEMICOLON);
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

    fn check_mem_reserve(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_mem_reserve)
    }

    #[test]
    fn check_simple_mem_reserve() {
        check_mem_reserve(
            "/memreserve/ 0x3000 0x4000;",
            r#"
RESERVE_MEMORY
  MEM_RESERVE "/memreserve/"
  WHITESPACE " "
  INT
    NUMBER "0x3000"
  WHITESPACE " "
  INT
    NUMBER "0x4000"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn error_tolerant_parsing() {
        check_generic_diag(
            &[
                Diagnostic::new(
                    TextRange::empty(TextSize::new(8)),
                    ErrorCode::Expected,
                    "Expecting SEMICOLON",
                ),
                Diagnostic::new(
                    TextRange::empty(TextSize::new(33)),
                    ErrorCode::Expected,
                    "Expecting SEMICOLON",
                ),
                Diagnostic::new(
                    TextRange::empty(TextSize::new(35)),
                    ErrorCode::Expected,
                    "Expecting SEMICOLON",
                ),
            ],
            "\
/dts-v1/

/ {
    some_prop = <5>
}",
            r#"
FILE
  HEADER
    DTS_V1 "/dts-v1/"
    WHITESPACE "\n\n"
  NODE
    DECORATION
    NAME
      SLASH "/"
    WHITESPACE " "
    NODE_BODY
      L_BRACE "{"
      WHITESPACE "\n    "
      PROPERTY
        DECORATION
        NAME
          IDENT "some_prop"
        WHITESPACE " "
        EQ "="
        WHITESPACE " "
        PROPERTY_LIST
          PROP_VALUE
            CELL
              CELL_INNER
                L_CHEV "<"
                INT
                  NUMBER "5"
                R_CHEV ">"
          WHITESPACE "\n"
      R_BRACE "}"
  
"#,
            Parser::parse_file,
        );
    }
}
