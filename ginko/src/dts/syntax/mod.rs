use std::fmt::{Display, Formatter};

mod cell;
pub mod expression;
mod file;
mod label;
pub mod node;
pub mod parser;
mod property;
mod reference;

pub use parser::Parser;

pub type SyntaxNode = rowan::SyntaxNode<Lang>;
pub type SyntaxToken = rowan::SyntaxToken<Lang>;
#[allow(unused)]
pub type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
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
    AT,            // @
    NUMBER,        // decimal or hex
    IDENT,         // simple identifier
    STRING,        // quoted string
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
    LABEL,               // label:
    NAME,                // Name of a node or property
    OP,                  // Operator Symbol
    INT,                 // Integer constant
    BINARY,              // A + B
    UNARY,               // ! A
    PAREN_EXPRESSION,    // ( expression )
    CELL,                // [optional decoration] < Cell content >
    CELL_INNER,          // < Cell content >
    BYTE_STRING,         // [ byte strings ]
    BYTE_CHUNK,          // a bunch of letters that make up a byte
    BITS_SPEC,           // /bits/ n specification
    DELETE_SPEC,         // /delete-property/ or /delete-node/
    OMIT_IF_NO_REF_SPEC, // /omit-if-no-ref/ spec
    HEADER,              // /dts-v1/ or /plugin/ header
    RESERVE_MEMORY,      // /memreserve/
    INCLUDE_FILE,        // /include/ file_name
    REF,                 // &name
    REF_PATH,            // &{path/to/somewhere}
    PATH,                // /path/to/somewhere
    PROPERTY_LIST,       // comma-separated property values
    PROP_VALUE,          // Property value, i.e., a cell, string, ...
    STRING_PROP,         // String as property value
    PROPERTY,            // A property, i.e., name = <value>;
    NODE,                // A Node, i.e., node { ... }
    DECORATION, // Additional attributes for a node or property. Currently, only /omit-if-no-ref/
    NODE_BODY,  // The body of a node; everything inside the curly braces
    FILE,
}

use SyntaxKind::*;

impl Display for SyntaxKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // TODO
        write!(f, "{:?}", self)
    }
}

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

#[cfg(test)]
mod testing {
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::lex::lex;
    use crate::dts::lex::token::Token;
    use crate::dts::syntax::SyntaxKind;
    use crate::dts::syntax::{Parser, SyntaxElement};
    use std::vec::IntoIter;

    pub fn str(element: SyntaxElement) -> String {
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

            SyntaxElement::Token(token) => {
                buffer.push_str(&format!("{:?} {:?}\n", kind, token.text()))
            }
        }
    }

    pub fn check_generic(
        expression: &str,
        expected: &str,
        parse_fn: impl FnOnce(&mut Parser<IntoIter<Token>>),
    ) {
        let (ast, errors) = Parser::new(lex(expression).into_iter()).parse(parse_fn);
        assert!(errors.is_empty(), "Got errors {:?}", errors);
        let ast_str = str(ast.into());
        let ast_str_trimmed = ast_str.trim();
        assert_eq!(ast_str_trimmed, expected.trim());
    }

    pub fn check_generic_diag(
        diagnostics: &[Diagnostic],
        expression: &str,
        expected: &str,
        parse_fn: impl FnOnce(&mut Parser<IntoIter<Token>>),
    ) {
        let (ast, errors) = Parser::new(lex(expression).into_iter()).parse(parse_fn);
        assert_eq!(errors, diagnostics);
        let ast_str = str(ast.into());
        let ast_str_trimmed = ast_str.trim();
        assert_eq!(ast_str_trimmed, expected.trim());
    }
}
