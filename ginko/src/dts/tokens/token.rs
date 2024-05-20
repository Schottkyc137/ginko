use crate::dts::data::HasSource;
use crate::dts::{HasSpan, Span};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::sync::Arc;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum TokenKind {
    Semicolon,
    // ;
    Slash,
    // /
    Equal,
    // =
    OpenBracket,
    // [
    CloseBracket,
    // ]
    OpenParen,
    // (
    CloseParen,
    // )
    ChevronLeft,
    // <
    ChevronRight,
    // >
    Comma,
    // ,
    OpenBrace,
    // {
    CloseBrace,
    // }
    Ident(String),
    // The most basic identifier, representing everything from node-name to byte string
    Label(String),
    String(String),
    // Since numbers can appear in various circumstances,
    // this simply represents a string starting with a number.
    // Verifying this number is done by the parser when more context is available.
    UnparsedNumber(String),
    Directive(CompilerDirective),
    Ref(Reference),
    Comment(String),
    Unknown(u8),
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum CompilerDirective {
    DTSVersionHeader,
    MemReserve,
    DeleteNode,
    DeleteProperty,
    Plugin,
    Bits,
    OmitIfNoRef,
    Include,
    Other(String),
}

impl Display for CompilerDirective {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerDirective::DTSVersionHeader => write!(f, "/dts-v1/"),
            CompilerDirective::MemReserve => write!(f, "/memreserve/"),
            CompilerDirective::DeleteNode => write!(f, "/delete-node/"),
            CompilerDirective::DeleteProperty => write!(f, "/delete-property/"),
            CompilerDirective::Plugin => write!(f, "/plugin/"),
            CompilerDirective::Bits => write!(f, "/bits/"),
            CompilerDirective::OmitIfNoRef => write!(f, "/omit-if-no-ref/"),
            CompilerDirective::Include => write!(f, "/include/"),
            CompilerDirective::Other(other) => write!(f, "/{other}/"),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Reference {
    // &some_label
    Simple(String),
    // &{/path/to/some/label}
    // Verification happens at the parser / analysis site
    Path(String),
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub source: Arc<Path>,
}

impl HasSpan for Token {
    fn span(&self) -> Span {
        self.span
    }
}

impl HasSource for Token {
    fn source(&self) -> Arc<Path> {
        self.source.clone()
    }
}
