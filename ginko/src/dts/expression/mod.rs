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
#[macro_use]
pub mod lex;
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

    DOUBLE_R_CHEV, // >>
    DOUBLE_L_CHEV, // <<

    R_CHEV, // >
    L_CHEV, // <

    R_BRAK, // [
    L_BRAK, // ]

    L_BRACE, // {
    R_BRACE, // }

    LTE, // <=
    GTE, // >=

    EQ, // =

    EQEQ, // ==
    NEQ,  // !=

    AMP,           // &
    CIRC,          // ^
    BAR,           // |
    LINE_COMMENT,  // //
    BLOCK_COMMENT, // /*

    DOUBLE_AMP, // &&
    DOUBLE_BAR, // ||

    QUESTION_MARK, // ?
    COLON,         // :
    SEMICOLON,     // ;
    COMMA,         // ,
    DOT,           // .
    UNDERSCORE,    // _
    POUND,         // #
    NUMBER,        // decimal or hex
    IDENT,         // simple identifier
    STRING,        // quoted string
    LABEL,         // label:
    // directives
    DTS_V1,          // /dts-v1/
    MEM_RESERVE,     // /memreserve/
    DELETE_NODE,     // /delete-node/
    DELETE_PROPERTY, // /delete-property/
    PLUGIN,          // /plugin/
    BITS,            // /bits/
    OMIT_IF_NO_REF,  // /omit-if-no-ref/
    INCLUDE,         // /include/

    ERROR,
    NAME,                // Name of a node or property
    OP,                  // Operator Symbol
    INT,                 // Integer constant
    BINARY,              // A + B
    UNARY,               // ! A
    PAREN_EXPRESSION,    // ( expression )
    CELL,                // < Cell content >
    BYTE_STRING,         // [ byte strings ]
    BYTE_CHUNK,          // a bunch of letters that make up a byte
    BITS_SPEC,           // /bits/ n specification
    DELETE_SPEC,         // /delete-property/ or /delete-node/
    OMIT_IF_NO_REF_SPEC, // /omit-if-no-ref/ spec
    HEADER,              // /dts-v1/ or /plugin/ header
    RESERVE_MEMORY,      // /memreserve/
    INCLUDE_FILE,        // /include/ file_name
    REFERENCE,           // &name
    PROPERTY_LIST,       // comma-separated property values
    PROP_VALUE,          // Property value, i.e., a cell, string, ...
    STRING_PROP,         // String as property value
    PROPERTY,            // A property, i.e., name = <value>;
    NODE,                // A Node, i.e., node { ... }
    DECORATION, // Additional attributes for a node or property. Currently, only /omit-if-no-ref/
    NODE_BODY,  // The body of a node; everything inside the curly braces
    FILE,
}

impl Display for SyntaxKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO
        write!(f, "{:?}", self)
    }
}

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
        assert!(raw.0 <= FILE as u16);
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
