pub mod token;

use crate::dts::lex::token::Token;
use crate::dts::syntax::SyntaxKind::*;

struct Lexer<'a> {
    contents: &'a [u8],
}

impl Lexer<'_> {
    fn peek(&self) -> Option<&u8> {
        self.contents.first()
    }

    fn consume(&mut self) -> Option<u8> {
        let ch = self.contents.first().copied()?;
        self.contents = &self.contents[1..];
        Some(ch)
    }

    pub fn new(contents: &str) -> Lexer<'_> {
        Lexer {
            contents: contents.as_bytes(),
        }
    }
}

impl Lexer<'_> {
    fn consume_ascii_string(&mut self, start: u8, s: &[u8]) -> Option<String> {
        if self.contents.starts_with(s) {
            let mut buf = String::with_capacity(8);
            buf.push(start as char);
            for _ in 0..s.len() {
                buf.push(self.consume().unwrap() as char)
            }
            Some(buf)
        } else {
            None
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let ch = self.consume()?;
        Some(match ch {
            b'(' => Token::new(L_PAR, "(".to_string()),
            b')' => Token::new(R_PAR, ")".to_string()),
            b'-' => Token::new(MINUS, "-".to_string()),
            b'~' => Token::new(TILDE, "~".to_string()),
            b'^' => Token::new(CIRC, "^".to_string()),
            b'!' => match self.peek() {
                Some(b'=') => {
                    self.consume();
                    Token::new(NEQ, "!=".to_string())
                }
                _ => Token::new(EXCLAMATION, "!".to_string()),
            },
            b'*' => Token::new(STAR, "*".to_string()),
            b'/' => match self.peek() {
                Some(b'/') => {
                    self.consume();
                    let mut buf = String::new();
                    buf.push_str("//");
                    while self.peek().is_some_and(|ch| ch != &b'\n') {
                        buf.push(self.consume().unwrap() as char)
                    }
                    Token::new(LINE_COMMENT, buf)
                }
                Some(b'*') => todo!(),
                // Kinda ugly. Is there a better solution?
                _ => {
                    if let Some(directive) = self.consume_ascii_string(ch, b"dts-v1/") {
                        Token::new(DTS_V1, directive)
                    } else if let Some(directive) = self.consume_ascii_string(ch, b"memreserve/") {
                        Token::new(MEM_RESERVE, directive)
                    } else if let Some(directive) = self.consume_ascii_string(ch, b"delete-node/") {
                        Token::new(DELETE_NODE, directive)
                    } else if let Some(directive) =
                        self.consume_ascii_string(ch, b"delete-property/")
                    {
                        Token::new(DELETE_PROPERTY, directive)
                    } else if let Some(directive) = self.consume_ascii_string(ch, b"plugin/") {
                        Token::new(PLUGIN, directive)
                    } else if let Some(directive) = self.consume_ascii_string(ch, b"bits/") {
                        Token::new(BITS, directive)
                    } else if let Some(directive) =
                        self.consume_ascii_string(ch, b"omit-if-no-ref/")
                    {
                        Token::new(OMIT_IF_NO_REF, directive)
                    } else if let Some(directive) = self.consume_ascii_string(ch, b"include/") {
                        Token::new(INCLUDE, directive)
                    } else {
                        Token::new(SLASH, "/".to_string())
                    }
                }
            },
            b',' => Token::new(COMMA, ",".to_string()),
            b'.' => Token::new(DOT, ".".to_string()),
            b';' => Token::new(SEMICOLON, ";".to_string()),
            b'#' => Token::new(POUND, "#".to_string()),
            b'%' => Token::new(PERCENT, "%".to_string()),
            b'+' => Token::new(PLUS, "+".to_string()),
            b':' => Token::new(COLON, ":".to_string()),
            b'?' => Token::new(QUESTION_MARK, "?".to_string()),
            b'[' => Token::new(L_BRAK, "[".to_string()),
            b']' => Token::new(R_BRAK, "]".to_string()),
            b'{' => Token::new(L_BRACE, "{".to_string()),
            b'}' => Token::new(R_BRACE, "}".to_string()),
            b'@' => Token::new(AT, "@".to_string()),
            b'>' => match self.peek() {
                Some(b'>') => {
                    self.consume();
                    Token::new(DOUBLE_R_CHEV, ">>".into())
                }
                Some(b'=') => {
                    self.consume();
                    Token::new(GTE, ">=".into())
                }
                _ => Token::new(R_CHEV, ">".into()),
            },
            b'<' => match self.peek() {
                Some(b'<') => {
                    self.consume();
                    Token::new(DOUBLE_L_CHEV, ">>".into())
                }
                Some(b'=') => {
                    self.consume();
                    Token::new(LTE, "<=".into())
                }
                _ => Token::new(L_CHEV, "<".into()),
            },
            b'=' => match self.peek() {
                Some(b'=') => {
                    self.consume();
                    Token::new(EQEQ, "==".into())
                }
                _ => Token::new(EQ, "=".into()),
            },
            b'&' => match self.peek() {
                Some(b'&') => {
                    self.consume();
                    Token::new(DOUBLE_AMP, "&&".into())
                }
                _ => Token::new(AMP, "&".into()),
            },
            b'|' => match self.peek() {
                Some(b'|') => {
                    self.consume();
                    Token::new(DOUBLE_BAR, "||".into())
                }
                _ => Token::new(BAR, "|".into()),
            },
            b' ' | b'\n' | b'\t' => {
                let mut buf = String::new();
                buf.push(ch as char);
                while self.peek().is_some_and(|ch| ch.is_ascii_whitespace()) {
                    buf.push(self.consume().unwrap() as char)
                }
                Token::new(WHITESPACE, buf)
            }
            b'0'..=b'9' => {
                let mut buf = String::new();
                buf.push(ch as char);
                while self.peek().is_some_and(|ch| ch.is_ascii_alphanumeric()) {
                    buf.push(self.consume().unwrap() as char)
                }
                Token::new(NUMBER, buf)
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let mut buf = String::new();
                buf.push(ch as char);
                while self
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == &b'_')
                {
                    buf.push(self.consume().unwrap() as char)
                }
                Token::new(IDENT, buf)
            }
            b'"' => {
                let mut buf = String::new();
                buf.push(ch as char);
                let mut is_escaped = false;
                while let Some(ch) = self.consume() {
                    buf.push(ch as char);
                    match ch {
                        b'\\' => is_escaped = !is_escaped,
                        b'"' if !is_escaped => break,
                        _ => is_escaped = false,
                    }
                }
                Token::new(STRING, buf)
            }
            _ => Token::new(ERROR, (ch as char).into()),
        })
    }
}

pub(crate) fn lex(input: &str) -> Vec<Token> {
    Lexer::new(input).collect()
}
