use crate::dts::expression::lex::lex;
use crate::dts::expression::token::Token;
use crate::dts::expression::SyntaxKind::*;
use crate::dts::expression::{NodeBuilder, SyntaxElement, SyntaxKind, SyntaxNode};
use itertools::Itertools;
use rowan::Checkpoint;
use std::iter::Peekable;

pub struct Parser<I: Iterator<Item = Token>> {
    builder: NodeBuilder,
    iter: Peekable<I>,
    errors: Vec<String>,
}

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn new(iter: I) -> Parser<I> {
        Parser {
            builder: NodeBuilder::new(),
            iter: iter.peekable(),
            errors: Vec::new(),
        }
    }
}

impl<I: Iterator<Item = Token>> Parser<I> {
    fn skip_ws(&mut self) {
        while self
            .iter
            .peek()
            .map(|token| token.is_whitespace())
            .unwrap_or(false)
        {
            self.bump();
        }
    }

    fn peek_kind(&mut self) -> Option<SyntaxKind> {
        self.skip_ws();
        self.iter.peek().map(|token| token.kind)
    }

    fn bump(&mut self) {
        if let Some(token) = self.iter.next() {
            self.builder.push(token);
        }
    }

    pub fn parse(mut self) -> (SyntaxNode, Vec<String>) {
        self.builder.start_node(ROOT);
        self.parse_expression();
        // eat all trailing whitespaces
        self.skip_ws();
        self.builder.finish_node();
        (SyntaxNode::new_root(self.builder.finish()), self.errors)
    }

    pub fn parse_primary(&mut self) {
        match self.peek_kind() {
            Some(L_PAR) => {
                self.builder.start_node(PAREN_EXPRESSION);
                self.bump();
                self.parse_expression();
                self.expect(&[R_PAR]);
                self.builder.finish_node();
            }
            Some(NUMBER) => {
                self.builder.start_node(INT);
                self.bump();
                self.builder.finish_node()
            }
            _ => {
                self.errors.push("Expecting number or '('".to_string());
                self.builder.start_node(ERROR);
                self.bump();
                self.builder.finish_node();
            }
        }
    }

    fn expect(&mut self, kinds: &[SyntaxKind]) {
        if self
            .peek_kind()
            .is_some_and(|peeked| kinds.contains(&peeked))
        {
            self.bump();
        } else {
            self.errors.push(format!(
                "Expecting {}",
                kinds.iter().map(|kind| kind.to_string()).join(", ")
            ));
            self.builder.start_node(ERROR);
            self.bump();
            self.builder.finish_node();
        }
    }

    pub fn parse_expression(&mut self) {
        let lhs = self.builder.checkpoint();
        self.parse_primary();
        self._parse_expression(lhs, 0);
    }

    fn _parse_expression(&mut self, lhs: Checkpoint, min_precedence: usize) {
        while let Some(precedence) = self.peek_kind().and_then(binary_precedence) {
            if precedence < min_precedence {
                break;
            }
            self.builder.start_node_at(lhs, BINARY);
            self.builder.start_node(OP);
            self.bump();
            self.builder.finish_node();
            let rhs = self.builder.checkpoint();
            self.parse_primary();
            while self
                .peek_kind()
                .and_then(binary_precedence)
                .is_some_and(|inner_precedence| inner_precedence > precedence)
            {
                self._parse_expression(rhs, precedence + 1);
            }
            self.builder.finish_node();
        }
    }
}

fn binary_precedence(kind: SyntaxKind) -> Option<usize> {
    Some(match kind {
        STAR | SLASH | PERCENT => 11,
        MINUS | PLUS => 10,
        DOUBLE_GT | DOUBLE_LT => 9,
        GT | LT | LTE | GTE => 8,
        EQ | NEQ => 7,
        AMP => 6,
        CIRC => 5,
        BAR => 4,
        DOUBLE_AMP => 3,
        DOUBLE_BAR => 2,
        QUESTION_MARK => 1,
        _ => return None,
    })
}

fn str(element: SyntaxElement) -> String {
    let mut buffer: String = String::new();
    _str(0, &mut buffer, element);
    buffer
}

fn _str(indent: usize, buffer: &mut String, element: SyntaxElement) {
    let kind: SyntaxKind = element.kind();
    buffer.push_str(&" ".repeat(indent));
    match element {
        SyntaxElement::Node(node) => {
            buffer.push_str(&format!("{:?}\n", kind));
            for child in node.children_with_tokens() {
                _str(indent + 2, buffer, child);
            }
        }

        SyntaxElement::Token(token) => buffer.push_str(&format!("{:?} {:?}\n", kind, token.text())),
    }
}

#[cfg(test)]
fn check(expression: &str, expected: &str) {
    let ast = Parser::new(lex(expression).into_iter()).parse();
    let ast_str = str(ast.0.into());
    let ast_str_trimmed = ast_str.trim();
    assert_eq!(ast_str_trimmed, expected.trim());
}

#[test]
fn check_primary() {
    check(
        "1",
        r#"
ROOT
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
ROOT
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
ROOT
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
ROOT
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
ROOT
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
