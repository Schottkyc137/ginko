use crate::dts::ast::CompilerDirective;
use crate::dts::data::{Position, Span};
use crate::dts::diagnostics::DiagnosticKind;
use crate::dts::reader::ByteReader;
use crate::dts::reader::Reader;
use crate::dts::{Diagnostic, HasSpan};
use std::sync::Arc;

enum LexerState {
    ExpectingNodeOrPropertyName,
    Other,
}

pub struct Lexer<R>
where
    R: Reader + Sized,
{
    reader: R,
    source: Arc<str>,
    state: LexerState,
    last_pos: Position,
}

impl Lexer<ByteReader> {
    pub fn from_text(text: impl Into<String>, source: Arc<str>) -> Lexer<ByteReader> {
        Lexer::new(ByteReader::from_string(text.into()), source)
    }
}

impl<R> Lexer<R>
where
    R: Reader + Sized,
{
    pub fn new(reader: R, source: Arc<str>) -> Lexer<R> {
        Lexer {
            reader,
            source,
            state: LexerState::ExpectingNodeOrPropertyName,
            last_pos: Position::zero(),
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
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub source: Arc<str>,
}

impl HasSpan for Token {
    fn span(&self) -> Span {
        self.span
    }
}

impl<R> Lexer<R>
where
    R: Reader + Sized,
{
    fn skip_whitespace(&mut self) {
        while self.reader.skip_if(|ch| ch.is_ascii_whitespace()) {}
    }

    fn read_while<F>(&mut self, cond: F) -> Vec<u8>
    where
        F: Fn(u8) -> bool,
    {
        let mut vec: Vec<u8> = vec![];
        while let Some(ch) = self.reader.peek() {
            if cond(ch) {
                vec.push(ch);
                self.reader.skip();
            } else {
                break;
            }
        }
        vec
    }

    pub fn pos(&self) -> Position {
        self.reader.pos()
    }

    pub fn source(&self) -> Arc<str> {
        self.source.clone()
    }

    // precondition: cursor is past '&' token
    // x = <&ref>
    //       ^~~ cursor is here
    fn path_or_reference(&mut self, start: Position) -> Option<Token> {
        let Some(ch) = self.reader.peek() else {
            return None;
        };
        match ch {
            b'{' => {
                self.reader.skip();
                let path = self.read_while(|ch| ch != b'}');
                let Some(_) = self.reader.consume() else {
                    return None;
                };
                Some(Token {
                    span: start.to(self.reader.pos()),
                    kind: TokenKind::Ref(Reference::Path(String::from_utf8(path).unwrap())),
                    source: self.source(),
                })
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                let label = self
                    .read_while(|ch| matches!(ch, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_'));
                let str = String::from_utf8(label).unwrap();
                Some(Token {
                    span: start.to(self.reader.pos()),
                    kind: TokenKind::Ref(Reference::Simple(str)),
                    source: self.source(),
                })
            }
            _ => Some(Token {
                span: start.to(self.reader.pos()),
                kind: TokenKind::Ref(Reference::Simple("".to_string())),
                source: self.source(),
            }),
        }
    }

    // precondition: cursor is past last slash
    // node,name = ...
    // ^~~ cursor is here
    fn ident_or_label(&mut self, start: Position) -> Token {
        let mut value = self.read_while(|ch| matches!(ch, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'?' | b'#' | b'-'));
        match self.reader.peek() {
            Some(b':') => {
                self.reader.skip();
                Token {
                    span: start.to(self.reader.pos()),
                    kind: TokenKind::Label(String::from_utf8(value).unwrap()),
                    source: self.source(),
                }
            }
            Some(b'@') => {
                self.reader.skip();
                value.push(b'@');
                value.extend(self.read_while(
                    |ch| matches!(ch, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'?' | b'#' | b'-')));
                Token {
                    span: start.to(self.reader.pos()),
                    kind: TokenKind::Ident(String::from_utf8(value).unwrap()),
                    source: self.source(),
                }
            }
            _ => Token {
                span: start.to(self.reader.pos()),
                kind: TokenKind::Ident(String::from_utf8(value).unwrap()),
                source: self.source(),
            },
        }
    }

    // precondition: cursor is past last slash
    // // comment
    //   ^~~ cursor is here
    fn line_comment(&mut self, pos: Position) -> Token {
        let comment = self.read_while(|ch| ch != b'\n');
        let comment_str = String::from_utf8(comment).unwrap();
        Token {
            span: pos.to(self.reader.pos()),
            kind: TokenKind::Comment(comment_str),
            source: self.source(),
        }
    }

    // precondition: cursor is past last slash
    // /* comment */
    //  ^~~ cursor is here
    fn multi_line_comment(&mut self, pos: Position) -> Option<Token> {
        let mut buf: Vec<u8> = vec![];
        loop {
            let Some(ch) = self.reader.consume() else {
                return None;
            };
            match ch {
                b'*' => {
                    if self.reader.peek() == Some(b'/') {
                        self.reader.skip();
                        let str = String::from_utf8(buf).unwrap();
                        return Some(Token {
                            span: pos.to(self.reader.pos()),
                            kind: TokenKind::Comment(str),
                            source: self.source(),
                        });
                    } else {
                        buf.push(ch)
                    }
                }
                ch => buf.push(ch),
            }
        }
    }

    // precondition: cursor is past first slash
    // /directive/;
    //  ^~~ cursor is here
    fn compiler_directive(&mut self, pos: Position) -> Token {
        let directive = self.read_while(|ch| ch != b'/');
        self.reader.skip();
        let directive_string = String::from_utf8(directive).unwrap();
        let directive = match directive_string.as_str() {
            "dts-v1" => CompilerDirective::DTSVersionHeader,
            "plugin" => CompilerDirective::Plugin,
            "memreserve" => CompilerDirective::MemReserve,
            "bits" => CompilerDirective::Bits,
            "delete-property" => CompilerDirective::DeleteProperty,
            "delete-node" => CompilerDirective::DeleteNode,
            "omit-if-no-ref" => CompilerDirective::OmitIfNoRef,
            "include" => CompilerDirective::Include,
            other => CompilerDirective::Other(other.into()),
        };
        Token {
            span: pos.to(self.reader.pos()),
            kind: TokenKind::Directive(directive),
            source: self.source(),
        }
    }

    // precondition: cursor is past first quote
    // "Hello, World!"
    //  ^~~ cursor is here
    fn string(&mut self, pos: Position) -> Option<Token> {
        let mut is_escaped: bool = false;
        let mut str = String::new();
        loop {
            let Some(ch) = self.reader.consume() else {
                return None;
            };
            match ch {
                b'\\' => {
                    if is_escaped {
                        str.push(b'\\' as char);
                        is_escaped = false;
                    } else {
                        is_escaped = true;
                    }
                }
                b'"' => {
                    if is_escaped {
                        str.push(b'"' as char);
                        is_escaped = false;
                    } else {
                        return Some(Token {
                            span: pos.to(self.reader.pos()),
                            kind: TokenKind::String(str),
                            source: self.source(),
                        });
                    }
                }
                _ => {
                    if is_escaped {
                        is_escaped = false
                    }
                    str.push(ch as char)
                }
            }
        }
    }

    fn number(&mut self, pos: Position) -> Token {
        let buf = self.read_while(|ch| ch.is_ascii_alphanumeric());
        let str = String::from_utf8(buf).unwrap();
        Token {
            span: pos.to(self.reader.pos()),
            kind: TokenKind::UnparsedNumber(str),
            source: self.source(),
        }
    }

    fn insert_pseudo_kind(&mut self, kind: TokenKind) {
        match kind {
            TokenKind::Semicolon => {
                self.state = LexerState::ExpectingNodeOrPropertyName;
            }
            TokenKind::Equal => {
                self.state = LexerState::Other;
            }
            TokenKind::CloseParen => {
                self.state = LexerState::ExpectingNodeOrPropertyName;
            }
            _ => {}
        }
    }

    #[cfg(test)]
    fn has_next(&self) -> bool {
        self.reader.peek().is_some()
    }

    fn consume(&mut self) -> Option<Token> {
        self.last_pos = self.pos();
        self.skip_whitespace();
        let Some(ch) = self.reader.peek() else {
            return None;
        };
        let start_pos = self.reader.pos();
        let source = self.source();
        let simple_token = |kind: TokenKind| -> Option<Token> {
            self.reader.skip();
            Some(Token {
                kind,
                span: start_pos.to(self.reader.pos()),
                source,
            })
        };
        use TokenKind::*;
        if let LexerState::ExpectingNodeOrPropertyName = self.state {
            if matches!(ch, b'a'..=b'z' | b'A'..=b'Z' | b',' | b'.' | b'_' | b'+' | b'-' | b'#' | b'?')
            {
                return Some(self.ident_or_label(start_pos));
            }
        }
        match ch {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => Some(self.ident_or_label(start_pos)),
            b'0'..=b'9' => Some(self.number(start_pos)),
            b'&' => {
                self.reader.skip();
                self.path_or_reference(start_pos)
            }
            b'"' => {
                self.reader.skip();
                self.string(start_pos)
            }
            b'/' => {
                self.reader.skip();
                let Some(next_ch) = self.reader.peek() else {
                    return Some(Token {
                        span: start_pos.to(self.reader.pos()),
                        kind: Slash,
                        source: self.source(),
                    });
                };
                match next_ch {
                    b'/' => {
                        self.reader.skip();
                        Some(self.line_comment(start_pos))
                    }
                    b'*' => {
                        self.reader.skip();
                        self.multi_line_comment(start_pos)
                    }
                    b'a'..=b'z' | b'A'..=b'Z' => Some(self.compiler_directive(start_pos)),
                    _ => Some(Token {
                        span: start_pos.to(self.reader.pos()),
                        kind: Slash,
                        source: self.source(),
                    }),
                }
            }
            b';' => {
                self.state = LexerState::ExpectingNodeOrPropertyName;
                simple_token(Semicolon)
            }
            b'=' => {
                self.state = LexerState::Other;
                simple_token(Equal)
            }
            b'[' => simple_token(OpenBracket),
            b']' => simple_token(CloseBracket),
            b'(' => simple_token(OpenParen),
            b')' => simple_token(CloseParen),
            b'{' => {
                self.state = LexerState::ExpectingNodeOrPropertyName;
                simple_token(OpenBrace)
            }
            b'}' => simple_token(CloseBrace),
            b'<' => simple_token(ChevronLeft),
            b'>' => simple_token(ChevronRight),
            b',' => simple_token(Comma),
            ch => simple_token(Unknown(ch)),
        }
    }
}

// This is a simple copy of the `Peekable` interface.
// We cannot just use the peekable interface because for operations
// like getting the EOF position, we still need a handle to the lexer.
pub struct PeekingLexer<R>
where
    R: Reader + Sized,
{
    lexer: Lexer<R>,
    peeked: Option<Option<Token>>,
}

impl<R> From<Lexer<R>> for PeekingLexer<R>
where
    R: Reader + Sized,
{
    fn from(value: Lexer<R>) -> Self {
        PeekingLexer {
            lexer: value,
            peeked: None,
        }
    }
}

impl<R> PeekingLexer<R>
where
    R: Reader + Sized,
{
    // Important: Only `peek` (and the Iterator implementation itself)
    // should call next on the lexer itself. Other implementations should
    // call next directly on the `PeekingLexer`
    pub fn peek(&mut self) -> Option<&Token> {
        let iter = &mut self.lexer;
        self.peeked.get_or_insert_with(|| iter.next()).as_ref()
    }

    pub fn peek_kind(&mut self) -> Option<&TokenKind> {
        self.peek().map(|tok| &tok.kind)
    }

    pub fn peek_expect(&mut self) -> Result<&Token, Diagnostic> {
        // something something cannot borrow as immutable something something
        // Therefore this is defined here as opposed to the `None` branch
        let eof_pos = self.lexer.pos();
        match self.peek() {
            None => Err(Diagnostic::new(
                eof_pos.as_span(),
                DiagnosticKind::UnexpectedEOF,
            )),
            Some(tok) => Ok(tok),
        }
    }

    pub fn expect(&mut self, kind: TokenKind) -> Result<Token, Diagnostic> {
        let prev_pos = self.lexer.pos();
        let next = self.next();
        match next {
            None => Err(Diagnostic::new(
                prev_pos.as_span(),
                DiagnosticKind::UnexpectedEOF,
            )),
            Some(token) => {
                if token.kind == kind {
                    Ok(token)
                } else {
                    Err(Diagnostic::new(
                        prev_pos.as_span(),
                        DiagnosticKind::Expected(vec![token.kind]),
                    ))
                }
            }
        }
    }

    pub fn expect_next(&mut self) -> Result<Token, Diagnostic> {
        let eof_pos = self.lexer.pos();
        let next = self.next();
        match next {
            None => Err(Diagnostic::new(
                eof_pos.as_span(),
                DiagnosticKind::UnexpectedEOF,
            )),
            Some(token) => Ok(token),
        }
    }

    pub fn insert_pseudo_kind(&mut self, kind: TokenKind) {
        self.lexer.insert_pseudo_kind(kind)
    }

    pub fn last_pos(&self) -> Position {
        self.lexer.last_pos
    }
}

impl<R> Iterator for PeekingLexer<R>
where
    R: Reader + Sized,
{
    type Item = Token;
    fn next(&mut self) -> Option<Token> {
        match self.peeked.take() {
            Some(v) => v,
            None => self.lexer.next(),
        }
    }
}

impl<R> Iterator for Lexer<R>
where
    R: Reader + Sized,
{
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        loop {
            let next = self.consume()?;
            if !matches!(next.kind, TokenKind::Comment(_)) {
                return Some(next);
            }
        }
    }
}

#[cfg(test)]
impl<R> Lexer<R>
where
    R: Reader + Sized,
{
    pub fn next_expect(&mut self) -> Token {
        self.next().expect("Unexpected EOF")
    }
}

#[cfg(test)]
mod test {
    use crate::dts::data::Position;
    use crate::dts::lexer::TokenKind::*;
    use crate::dts::lexer::{CompilerDirective, Lexer, Reference, Token};
    use itertools::Itertools;
    use std::sync::Arc;

    fn tokenize_fully(string: impl Into<std::string::String>) -> (Vec<Token>, Arc<str>) {
        let source: Arc<str> = "inline_source".into();
        (
            Lexer::from_text(string.into(), source.clone()).collect(),
            source,
        )
    }

    #[test]
    pub fn tokenize_simple_characters() {
        let (tokens, source) = tokenize_fully("; = [] ><");
        assert_eq!(
            tokens,
            vec![
                Token {
                    kind: Semicolon,
                    span: Position::zero().to(Position::new(0, 1)),
                    source: source.clone(),
                },
                Token {
                    kind: Equal,
                    span: Position::new(0, 2).to(Position::new(0, 3)),
                    source: source.clone(),
                },
                Token {
                    kind: OpenBracket,
                    span: Position::new(0, 4).to(Position::new(0, 5)),
                    source: source.clone(),
                },
                Token {
                    kind: CloseBracket,
                    span: Position::new(0, 5).to(Position::new(0, 6)),
                    source: source.clone(),
                },
                Token {
                    kind: ChevronRight,
                    span: Position::new(0, 7).to(Position::new(0, 8)),
                    source: source.clone(),
                },
                Token {
                    kind: ChevronLeft,
                    span: Position::new(0, 8).to(Position::new(0, 9)),
                    source: source.clone(),
                },
            ]
        )
    }

    #[test]
    pub fn tokenize_reference() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("&ref", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 4)),
                kind: Ref(Reference::Simple("ref".into())),
                source: source.clone(),
            }
        );
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("&ref0_from_4", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 12)),
                kind: Ref(Reference::Simple("ref0_from_4".into())),
                source: source.clone(),
            }
        )
    }

    #[test]
    pub fn tokenize_node_name() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("node@addr", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 9)),
                kind: Ident("node@addr".into()),
                source: source.clone(),
            }
        );
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("node@addr@", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 9)),
                kind: Ident("node@addr".into()),
                source: source.clone(),
            }
        );
        assert_eq!(
            lexer.next(),
            Some(Token {
                span: Position::new(0, 9).as_char_span(),
                kind: Unknown(b'@'),
                source: source.clone(),
            })
        )
    }

    #[test]
    pub fn error_on_wrong_reference() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("&0ref", source.clone());
        assert_eq!(
            lexer.next(),
            Some(Token {
                kind: Ref(Reference::Simple("".to_string())),
                span: Position::zero().as_char_span(),
                source: source.clone(),
            })
        );

        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("& ref", source.clone());
        assert_eq!(
            lexer.next(),
            Some(Token {
                kind: Ref(Reference::Simple("".to_string())),
                span: Position::zero().as_char_span(),
                source: source.clone(),
            })
        );
    }

    #[test]
    pub fn tokenize_path_reference() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("&{}", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 3)),
                kind: Ref(Reference::Path("".into())),
                source: source.clone(),
            }
        );

        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("&{ref}", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 6)),
                kind: Ref(Reference::Path("ref".into())),
                source: source.clone(),
            }
        );

        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("&{/}", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 4)),
                kind: Ref(Reference::Path("/".into())),
                source: source.clone(),
            }
        );

        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("&{/path/to/node}", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 16)),
                kind: Ref(Reference::Path("/path/to/node".into())),
                source: source.clone(),
            }
        );
    }

    #[test]
    pub fn tokenize_property_name() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("fsbl,my_node#s", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 14)),
                kind: Ident("fsbl,my_node#s".into()),
                source: source.clone(),
            }
        );
    }

    #[test]
    pub fn tokenize_labels() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("my_label:", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 9)),
                kind: Label("my_label".into()),
                source: source.clone(),
            }
        );

        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("my_label#:", source.clone());
        assert_eq!(
            lexer.next_expect(),
            Token {
                span: Position::zero().to(Position::new(0, 10)),
                kind: Label("my_label#".into()),
                source: source.clone(),
            }
        );
    }

    #[test]
    pub fn tokenize_comment() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text(
            "something // &hshg chars
next_token",
            source.clone(),
        );
        let mut tokens: Vec<Token> = vec![];
        while lexer.has_next() {
            tokens.push(lexer.consume().expect("Unexpected EOF"))
        }
        assert_eq!(
            tokens,
            vec![
                Token {
                    span: Position::zero().to(Position::new(0, 9)),
                    kind: Ident("something".into()),
                    source: source.clone(),
                },
                Token {
                    span: Position::new(0, 10).char_to(24),
                    kind: Comment(" &hshg chars".into()),
                    source: source.clone(),
                },
                Token {
                    span: Position::new(1, 0).char_to(10),
                    kind: Ident("next_token".into()),
                    source: source.clone(),
                },
            ]
        );
    }

    #[test]
    pub fn tokenize_multiline_comment() {
        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text("token /* some comment */ token", source.clone());
        let mut tokens: Vec<Token> = vec![];
        while lexer.has_next() {
            tokens.push(lexer.consume().expect("Unexpected EOF"))
        }
        assert_eq!(
            tokens,
            vec![
                Token {
                    span: Position::zero().to(Position::new(0, 5)),
                    kind: Ident("token".into()),
                    source: source.clone(),
                },
                Token {
                    span: Position::new(0, 6).char_to(24),
                    kind: Comment(" some comment ".into()),
                    source: source.clone(),
                },
                Token {
                    span: Position::new(0, 25).char_to(30),
                    kind: Ident("token".into()),
                    source: source.clone(),
                },
            ]
        );

        let source: Arc<str> = "inline source".into();
        let mut lexer = Lexer::from_text(
            "/* first line
second line
third line
*/
token",
            source.clone(),
        );
        let mut tokens: Vec<Token> = vec![];
        while lexer.has_next() {
            tokens.push(lexer.consume().expect("Unexpected EOF"))
        }
        assert_eq!(
            tokens,
            vec![
                Token {
                    span: Position::zero().to(Position::new(3, 2)),
                    kind: Comment(
                        " first line
second line
third line
"
                        .into()
                    ),
                    source: source.clone(),
                },
                Token {
                    span: Position::new(4, 0).char_to(5),
                    kind: Ident("token".into()),
                    source: source.clone(),
                },
            ]
        );
    }

    #[test]
    fn compiler_directive() {
        let source: Arc<str> = "inline source".into();
        assert_eq!(
            Lexer::from_text("/dts-v1/", source.clone()).next_expect(),
            Token {
                span: Position::zero().char_to(8),
                kind: Directive(CompilerDirective::DTSVersionHeader),
                source: source.clone(),
            }
        );

        let source: Arc<str> = "inline source".into();
        assert_eq!(
            Lexer::from_text("/undefined/", source.clone()).next_expect(),
            Token {
                span: Position::zero().char_to(11),
                kind: Directive(CompilerDirective::Other("undefined".into())),
                source: source.clone(),
            }
        )
    }

    #[test]
    pub fn strings() {
        let strings = [
            (r#""""#, ""),
            (r#""ns16550""#, "ns16550"),
            (r#""Hello, World!""#, "Hello, World!"),
            (r#""\"foo\"""#, r#""foo""#),
            (r#""foo \"bar\"""#, r#"foo "bar""#),
            (r#""foo \\bar""#, r#"foo \bar"#),
        ];

        for (raw_str, expected) in strings {
            let source: Arc<str> = "inline source".into();
            assert_eq!(
                Lexer::from_text(raw_str, source.clone()).next_expect(),
                Token {
                    span: Position::zero().char_to(raw_str.len() as u32),
                    kind: String(expected.into()),
                    source: source.clone(),
                }
            )
        }
    }

    #[test]
    pub fn conflicting_node_names() {
        let source: Arc<str> = "inline source".into();
        let str1 = "some_node { ,property-name = <1>,<2>; };";
        let tokens = Lexer::from_text(str1, source.clone())
            .map(|tok| tok.kind)
            .collect_vec();
        assert_eq!(
            tokens,
            vec![
                Ident("some_node".into()),
                OpenBrace,
                Ident(",property-name".into()),
                Equal,
                ChevronLeft,
                UnparsedNumber("1".into()),
                ChevronRight,
                Comma,
                ChevronLeft,
                UnparsedNumber("2".into()),
                ChevronRight,
                Semicolon,
                CloseBrace,
                Semicolon,
            ]
        )
    }
}
