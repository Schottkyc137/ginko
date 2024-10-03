/// Devicetree expression parsing
///
/// The following BNF syntax describes the allowed expressions in a Devicetree context.
///```ebnf
/// <expression> ::= <logical-or-expression>
///                | <logical-or-expression> ? <expression> : <expression>
/// <logical-or-expression> ::= <logical-and-expression>
///                           | <logical-or-expression> || <logical-and-expression>
/// <logical-and-expression> ::= <inclusive-or-expression>
///                            | <logical-and-expression> && <inclusive-or-expression>
/// <inclusive-or-expression> ::= <exclusive-or-expression>
///                             | <inclusive-or-expression> | <exclusive-or-expression>
///
/// <exclusive-or-expression> ::= <and-expression>
///                             | <exclusive-or-expression> ^ <and-expression>
///
/// <and-expression> ::= <equality-expression>
///                    | <and-expression> & <equality-expression>
///
/// <equality-expression> ::= <relational-expression>
///                         | <equality-expression> == <relational-expression>
///                         | <equality-expression> != <relational-expression>
///
/// <relational-expression> ::= <shift-expression>
///                           | <relational-expression> < <shift-expression>
///                           | <relational-expression> > <shift-expression>
///                           | <relational-expression> <= <shift-expression>
///                           | <relational-expression> >= <shift-expression>
///
/// <shift-expression> ::= <additive-expression>
///                      | <shift-expression> << <additive-expression>
///                      | <shift-expression> >> <additive-expression>
///
/// <additive-expression> ::= <multiplicative-expression>
///                         | <additive-expression> + <multiplicative-expression>
///                         | <additive-expression> - <multiplicative-expression>
///
/// <multiplicative-expression> ::= <unary-expression>
///                               | <multiplicative-expression> * <unary-expression>
///                               | <multiplicative-expression> / <unary-expression>
///                               | <multiplicative-expression> % <unary-expression>
///
/// <unary-expression> ::= <primary-expression>
///                      | <unary-operator> <unary-expression>
///
/// <primary-expression> ::= <constant> | ( <expression> )
///
/// <constant> ::= <integer-constant> | <character-constant>
///
/// <unary-operator> ::= | -
///                    | ~
///                    | !
///```
pub mod ast;
mod eval;
pub mod lex;
pub mod parser;
pub mod token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types)]
#[repr(u16)]
pub enum SyntaxKind {
    WHITESPACE = 0,

    L_PAR,       // (
    R_PAR,       // )
    MINUS,       // -
    TILDE,       // ~
    EXCLAMATION, // !

    STAR,    // *
    SLASH,   // /
    PERCENT, // %

    PLUS, // +

    DOUBLE_GT, // >>
    DOUBLE_LT, // <<

    GT, // >
    LT, // <

    LTE, // <=
    GTE, // >=

    EQ,  // ==
    NEQ, // !=

    AMP,  // &
    CIRC, // ^
    BAR,  // |

    DOUBLE_AMP, // &&
    DOUBLE_BAR, // ||

    QUESTION_MARK, // ?
    COLON,         // :
    NUMBER,        // decimal or hex

    ERROR,
    OP,               // Operator Symbol
    INT,              // Integer constant
    BINARY,           // A + B
    UNARY,            // ! A
    PAREN_EXPRESSION, // ( expression )
}

impl Display for SyntaxKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO
        write!(f, "{:?}", self)
    }
}

use crate::dts::expression::token::Token;
use rowan::{Checkpoint, GreenNode, GreenNodeBuilder};
use std::fmt::{Display, Formatter};
use SyntaxKind::*;

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {}
impl rowan::Language for Lang {
    type Kind = SyntaxKind;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        assert!(raw.0 <= PAREN_EXPRESSION as u16);
        unsafe { std::mem::transmute::<u16, SyntaxKind>(raw.0) }
    }
    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

type SyntaxNode = rowan::SyntaxNode<Lang>;
#[allow(unused)]
type SyntaxToken = rowan::SyntaxToken<Lang>;
#[allow(unused)]
type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

struct NodeBuilder(GreenNodeBuilder<'static>);

impl NodeBuilder {
    pub fn new() -> NodeBuilder {
        NodeBuilder(GreenNodeBuilder::new())
    }

    pub fn push(&mut self, value: Token) {
        self.0.token(value.kind.into(), value.value.as_str())
    }

    pub fn finish(self) -> GreenNode {
        self.0.finish()
    }

    pub fn start_node(&mut self, node: SyntaxKind) {
        self.0.start_node(node.into())
    }

    pub fn finish_node(&mut self) {
        self.0.finish_node()
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.0.checkpoint()
    }

    pub fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind) {
        self.0.start_node_at(checkpoint, kind.into())
    }
}
