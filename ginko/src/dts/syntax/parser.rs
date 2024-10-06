use crate::dts::lex::token::Token;
use crate::dts::syntax::SyntaxKind;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::SyntaxNode;
use itertools::Itertools;
use rowan::{Checkpoint, GreenNodeBuilder};
use std::iter::Peekable;

pub struct Parser<I: Iterator<Item = Token>> {
    builder: GreenNodeBuilder<'static>,
    iter: Peekable<I>,
    errors: Vec<String>,
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn new(iter: I) -> Parser<I> {
        Parser {
            builder: GreenNodeBuilder::new(),
            iter: iter.peekable(),
            errors: Vec::new(),
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

    fn duplicates_error(&mut self, kinds: &[SyntaxKind]) {
        self.error_token(format!(
            "Expecting {}",
            kinds.iter().map(|kind| kind.to_string()).join(", ")
        ))
    }

    pub(crate) fn expect(&mut self, kinds: SyntaxKind) {
        if self.peek_kind().is_some_and(|peeked| peeked == kinds) {
            self.bump();
        } else {
            self.duplicates_error(&[kinds]);
        }
    }

    pub(crate) fn expect_direct(&mut self, kind: SyntaxKind) {
        if self.peek_kind_direct().is_some_and(|peeked| kind == peeked) {
            self.bump();
        } else {
            self.duplicates_error(&[kind]);
        }
    }

    pub(crate) fn bump(&mut self) {
        if let Some(token) = self.iter.next() {
            self.builder.token(token.kind.into(), token.value.as_str());
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
        self.errors.push(message.into());
        self.start_node(ERROR);
        self.bump();
        self.finish_node();
    }

    pub(crate) fn eof_error(&mut self) {
        self.errors.push("Unexpected EOF".to_string())
    }
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse(mut self, target: impl FnOnce(&mut Parser<I>)) -> (SyntaxNode, Vec<String>) {
        target(&mut self);
        // eat all trailing whitespaces
        self.skip_ws();
        (SyntaxNode::new_root(self.builder.finish()), self.errors)
    }
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_reference(&mut self) {
        assert_eq!(self.peek_kind(), Some(AMP));
        self.start_node(REFERENCE);
        self.bump();
        self.expect_direct(IDENT);
        self.finish_node();
    }

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
    use crate::dts::syntax::testing::check_generic;
    use crate::dts::syntax::Parser;

    fn check_reference(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_reference)
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
}
