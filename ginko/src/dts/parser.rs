use crate::dts::ast::{
    AnyDirective, Cell, DtsFile, Include, Memreserve, Node, NodeItem, NodeName, NodePayload, Path,
    Primary, Property, PropertyValue, ReferencedNode, WithToken,
};
use crate::dts::data::{HasSource, Span};
use crate::dts::diagnostics::{Diagnostic, NameContext};
use crate::dts::error_codes::ErrorCode;
use crate::dts::lexer::{Lexer, PeekingLexer, Reference, Token, TokenKind};
use crate::dts::reader::{ByteReader, Reader};
use crate::dts::{CompilerDirective, HasSpan};
use itertools::Itertools;
use std::path::Path as StdPath;
use std::sync::Arc;

/// The `Parser` class is responsible for syntactical analysis,
/// transforming the input token stream into an AST.
pub struct Parser<R>
where
    R: Reader + Sized,
{
    lexer: PeekingLexer<R>,
    pub diagnostics: Vec<Diagnostic>,
}

type Result<T> = std::result::Result<T, Diagnostic>;

impl<R> Parser<R>
where
    R: Reader + Sized,
{
    pub fn new(lexer: Lexer<R>) -> Parser<R> {
        Parser {
            lexer: PeekingLexer::from(lexer),
            diagnostics: vec![],
        }
    }
}

impl Parser<ByteReader> {
    pub fn from_text(text: impl Into<String>, source: Arc<StdPath>) -> Parser<ByteReader> {
        let lexer = Lexer::from_text(text, source);
        Parser {
            lexer: lexer.into(),
            diagnostics: vec![],
        }
    }
}

impl<R> Parser<R>
where
    R: Reader + Sized,
{
    pub fn diagnostics(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter()
    }

    fn skip_tok(&mut self) {
        let _ = self.lexer.next();
    }

    fn check_ident<F1, F2>(
        &mut self,
        span: Span,
        str: &str,
        predicate: F1,
        starting_chars: F2,
        name_context: NameContext,
    ) where
        F1: Fn(char) -> bool,
        F2: FnMut(char) -> bool,
    {
        if let Some((pos, ch)) = str.chars().enumerate().find(|(_, ch)| !predicate(*ch)) {
            self.diagnostics.push(Diagnostic::new(
                span.start().offset_by_char(pos as i32).as_char_span(),
                self.lexer.source(),
                ErrorCode::IllegalChar,
                format!("Illegal char '{ch}' in {name_context}"),
            ));
        } else if str.len() > 31 {
            self.diagnostics.push(Diagnostic::new(
                span,
                self.lexer.source(),
                ErrorCode::NameTooLong,
                format!(
                    "{name_context} should only have 31 characters but has {} characters",
                    str.len()
                ),
            ))
        } else if str.is_empty() {
            self.diagnostics.push(Diagnostic::new(
                span,
                self.lexer.source(),
                ErrorCode::ExpectedName,
                format!("Expected {name_context}"),
            ))
        } else if !str.starts_with(starting_chars) {
            self.diagnostics.push(Diagnostic::new(
                span,
                self.lexer.source(),
                ErrorCode::IllegalStart,
                format!(
                    "{name_context} may not start with {}",
                    str.chars().next().unwrap()
                ),
            ))
        }
    }

    fn check_is_label(&mut self, span: Span, str: &str) {
        self.check_ident(
            span,
            str,
            |ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'),
            |ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '_'),
            NameContext::Label,
        )
    }

    fn check_is_node_name(&mut self, span: Span, name: &NodeName) {
        if name.unit_address.is_none() && name.name == "__symbols__" {
            return;
        }
        self.check_ident(
            span,
            name.name.as_str(),
            |ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | ',' | '.' | '_' | '+' | '-'),
            |ch| ch.is_ascii_alphabetic(),
            NameContext::NodeName,
        );
        if let Some(unit_address) = &name.unit_address {
            self.check_ident(
                span,
                unit_address.as_str(),
                |ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | ',' | '.' | '_' | '+' | '-'),
                |_| true,
                NameContext::NodeName,
            );
        }
    }

    fn check_is_property_name(&mut self, span: Span, str: &str) {
        self.check_ident(
            span,
            str,
            |ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | ',' | '.' | '_' | '+' | '-' | '?' | '#'),
            |_| true,
            NameContext::PropertyName,
        );
    }

    fn reference(
        &mut self,
        token: Token,
        reference: &Reference,
    ) -> WithToken<crate::dts::ast::Reference> {
        match reference {
            Reference::Simple(reference) => {
                self.check_is_label(token.span, reference);
                WithToken::new(crate::dts::ast::Reference::Label(reference.clone()), token)
            }
            Reference::Path(path) => {
                if path.is_empty() {
                    self.diagnostics.push(Diagnostic::from_token(
                        token.clone(),
                        ErrorCode::PathCannotBeEmpty,
                        "Path cannot be empty",
                    ));
                }
                let path = Path::from(path.as_str());
                for el in path.iter() {
                    self.check_is_node_name(token.span(), el);
                }
                WithToken::new(crate::dts::ast::Reference::Path(path), token)
            }
        }
    }

    fn number_u32(&mut self) -> Result<Cell> {
        let tok = self.lexer.expect_next()?;
        match &tok.kind {
            TokenKind::Ref(reference) => {
                Ok(Cell::Reference(self.reference(tok.clone(), reference)))
            }
            TokenKind::UnparsedNumber(num) => {
                let num = if num.len() == 1 {
                    num.parse::<u32>()
                } else if num.starts_with("0x") {
                    u32::from_str_radix(&num.as_str()[2..], 16)
                } else if num.starts_with('0') {
                    u32::from_str_radix(&num.as_str()[1..], 8)
                } else {
                    num.parse::<u32>()
                };
                match num {
                    Ok(num) => Ok(Cell::Number(WithToken::new(num, tok))),
                    Err(err) => {
                        self.diagnostics.push(Diagnostic::parse_int_error(
                            tok.span(),
                            tok.source(),
                            err,
                        ));
                        Ok(Cell::Number(WithToken::new(0, tok)))
                    }
                }
            }
            TokenKind::OpenParen => {
                self.skip_expression_starting_with_paren();
                if self.lexer.peek().is_none() {
                    self.diagnostics.push(Diagnostic::from_token(
                        tok,
                        ErrorCode::UnbalancedParentheses,
                        "Unbalanced parentheses",
                    ));
                }
                Ok(Cell::Expression)
            }
            _ => Err(Diagnostic::expected(
                tok.span(),
                tok.source(),
                &[
                    TokenKind::UnparsedNumber("".to_string()),
                    TokenKind::Ref(Reference::Simple("".to_string())),
                    TokenKind::OpenParen,
                ],
            )),
        }
    }

    fn memreserve_number_u64(&mut self) -> Result<WithToken<u64>> {
        let tok = self.lexer.expect_next()?;
        match &tok.kind {
            TokenKind::UnparsedNumber(num) => {
                let num = if num.len() == 1 {
                    num.parse::<u64>()
                } else if num.starts_with("0x") {
                    u64::from_str_radix(&num.as_str()[2..], 16)
                } else if num.starts_with('0') {
                    u64::from_str_radix(&num.as_str()[1..], 8)
                } else {
                    num.parse::<u64>()
                };
                match num {
                    Ok(num) => Ok(WithToken::new(num, tok)),
                    Err(err) => {
                        self.diagnostics.push(Diagnostic::parse_int_error(
                            tok.span(),
                            tok.source(),
                            err,
                        ));
                        Ok(WithToken::new(0, tok))
                    }
                }
            }
            _ => Err(Diagnostic::expected(
                self.lexer.last_pos().offset_by_char(1).as_span(),
                self.lexer.source(),
                &[TokenKind::UnparsedNumber("".to_string())],
            )),
        }
    }

    fn byte_string(&mut self) -> Result<WithToken<Vec<u8>>> {
        let tok = self.lexer.expect_next()?;
        match &tok.kind {
            TokenKind::UnparsedNumber(raw_str) | TokenKind::Ident(raw_str) => {
                if raw_str.len() % 2 != 0 {
                    self.diagnostics.push(Diagnostic::from_token(
                        tok.clone(),
                        ErrorCode::OddNumberOfBytestringElements,
                        "Number of elements in byte string must be even",
                    ));
                    return Ok(WithToken::new(vec![], tok));
                }
                let mut bytes: Vec<u8> = Vec::with_capacity(raw_str.len() / 2);
                for (first, second) in raw_str.bytes().map(|ch| ch.to_ascii_lowercase()).tuples() {
                    match u8::from_str_radix(std::str::from_utf8(&[first, second]).unwrap(), 16) {
                        Ok(byte) => bytes.push(byte),
                        Err(err) => {
                            self.diagnostics.push(Diagnostic::parse_int_error(
                                tok.span(),
                                tok.source(),
                                err,
                            ));
                            return Ok(WithToken::new(vec![], tok));
                        }
                    }
                }
                Ok(WithToken::new(bytes, tok))
            }
            _ => {
                self.diagnostics.push(Diagnostic::expected(
                    tok.span(),
                    tok.source(),
                    &[
                        TokenKind::UnparsedNumber("".to_string()),
                        TokenKind::Ident("".to_string()),
                    ],
                ));
                Ok(WithToken::new(vec![], tok))
            }
        }
    }

    fn skip_expression_starting_with_paren(&mut self) {
        let mut depth = 1;
        loop {
            let peeked = self.lexer.peek();
            if matches!(
                peeked,
                Some(Token {
                    kind: TokenKind::OpenParen,
                    ..
                })
            ) {
                depth += 1;
                self.skip_tok();
            } else if matches!(
                peeked,
                Some(Token {
                    kind: TokenKind::CloseParen,
                    ..
                })
            ) {
                depth -= 1;
                self.skip_tok();
                if depth == 0 {
                    return;
                }
            } else if peeked.is_none() {
                return;
            } else {
                self.skip_tok();
            }
        }
    }

    pub fn property_value(&mut self) -> Result<PropertyValue> {
        if matches!(
            self.lexer.peek(),
            Some(Token {
                kind: TokenKind::Directive(CompilerDirective::Bits),
                ..
            })
        ) {
            self.skip_tok();
            let width = self.lexer.expect_next()?;
            if !matches!(width.kind, TokenKind::UnparsedNumber(_)) {
                self.diagnostics.push(Diagnostic::expected(
                    width.span(),
                    width.source(),
                    &[TokenKind::UnparsedNumber("".to_string())],
                ))
            }
        }
        let tok = self.lexer.expect_next()?;
        match &tok.kind {
            TokenKind::String(string) => {
                Ok(PropertyValue::String(WithToken::new(string.clone(), tok)))
            }
            TokenKind::ChevronLeft => {
                let mut cells: Vec<Cell> = vec![];
                let end_tok: Token;
                loop {
                    self.skip_optional_label();
                    if self.lexer.peek_expect()?.kind == TokenKind::ChevronRight {
                        end_tok = self.lexer.expect_next()?;
                        break;
                    }

                    match self.number_u32() {
                        Ok(numb) => cells.push(numb),
                        Err(err) => self.diagnostics.push(err),
                    }
                }
                Ok(PropertyValue::Cells(tok, cells, end_tok))
            }
            TokenKind::Ref(reference) => Ok(PropertyValue::Reference(
                self.reference(tok.clone(), reference),
            )),
            TokenKind::OpenBracket => {
                let mut byte_strings: Vec<WithToken<Vec<u8>>> = vec![];
                let end: Token;
                loop {
                    self.skip_optional_label();
                    let tok = self.lexer.peek_expect()?;
                    if tok.kind == TokenKind::CloseBracket {
                        end = tok.clone();
                        self.skip_tok();
                        break;
                    }
                    byte_strings.push(self.byte_string()?);
                }
                Ok(PropertyValue::ByteStrings(tok, byte_strings, end))
            }
            _ => Err(Diagnostic::expected(
                tok.span(),
                tok.source(),
                &[
                    TokenKind::String("".to_string()),
                    TokenKind::ChevronLeft,
                    TokenKind::Ref(Reference::Simple("".to_string())),
                    TokenKind::OpenBracket,
                ],
            )),
        }
    }

    fn skip_optional_label(&mut self) {
        if matches!(
            self.lexer.peek(),
            Some(Token {
                kind: TokenKind::Label(_),
                ..
            })
        ) {
            self.skip_tok();
        }
    }

    pub fn property_values(&mut self) -> Result<Vec<PropertyValue>> {
        let mut values: Vec<PropertyValue> = vec![];
        loop {
            self.skip_optional_label();
            values.push(self.property_value()?);
            self.skip_optional_label();
            if self.lexer.peek_expect()?.kind == TokenKind::Comma {
                self.skip_tok();
            } else {
                break;
            }
        }
        Ok(values)
    }

    pub fn property_name(&mut self) -> Result<WithToken<String>> {
        let tok = self.lexer.expect_next()?;
        if let TokenKind::Ident(value) = tok.kind.clone() {
            self.check_is_property_name(tok.span(), &value);
            return Ok(WithToken::new(value, tok));
        }
        Err(Diagnostic::expected(
            tok.span(),
            tok.source(),
            &[TokenKind::Ident("".to_string())],
        ))
    }

    pub fn node_name(&mut self) -> Result<WithToken<NodeName>> {
        let tok = self.lexer.expect_next()?;
        if let TokenKind::Ident(value) = tok.kind.clone() {
            let node_name = NodeName::from(value);
            self.check_is_node_name(tok.span(), &node_name);
            return Ok(WithToken::new(node_name, tok));
        }
        Err(Diagnostic::expected(
            tok.span(),
            tok.source(),
            &[TokenKind::Ident("".to_string())],
        ))
    }

    pub fn node_payload(&mut self) -> Result<NodePayload> {
        let end: Token;
        self.lexer.expect(TokenKind::OpenBrace)?;
        let mut items: Vec<NodeItem> = vec![];
        let mut node_discovered = false;
        loop {
            let tok = self.lexer.expect_next()?;
            if tok.kind == TokenKind::Directive(CompilerDirective::DeleteNode) {
                let node_name = self.node_name()?;
                self.expect_semicolon()?;
                items.push(NodeItem::DeletedNode(tok, node_name));
                continue;
            }
            if tok.kind == TokenKind::Directive(CompilerDirective::DeleteProperty) {
                let property_name = self.property_name()?;
                self.expect_semicolon()?;
                items.push(NodeItem::DeletedProperty(tok, property_name));
                continue;
            }
            let (tok, label) = match &tok.kind {
                TokenKind::Label(string) => {
                    self.check_is_label(tok.span(), string);
                    (
                        self.lexer.expect_next()?,
                        Some(WithToken::new(string.clone(), tok)),
                    )
                }
                _ => (tok, None),
            };
            let ident = match &tok.kind {
                TokenKind::Ident(string) => WithToken::new(string.clone(), tok),
                TokenKind::CloseBrace => {
                    end = self.expect_semicolon()?.unwrap_or(tok.clone());
                    break;
                }
                _ => {
                    return Err(Diagnostic::expected(
                        tok.span(),
                        tok.source(),
                        &[TokenKind::Ident("".to_string()), TokenKind::CloseBrace],
                    ));
                }
            };
            let tok = self.lexer.peek_expect()?;
            let cloned_tok = tok.clone();
            match &tok.kind {
                TokenKind::OpenBrace => {
                    node_discovered = true;
                    let node_name: WithToken<NodeName> = From::from(ident);
                    self.check_is_node_name(node_name.span(), &node_name);
                    let payload = self.node_payload()?;
                    items.push(NodeItem::Node(Arc::new(Node {
                        name: node_name,
                        label,
                        payload,
                    })));
                }
                TokenKind::Equal => {
                    self.skip_tok();
                    self.check_is_property_name(ident.span(), &ident);
                    let values = self.property_values()?;
                    let end_tok = self.expect_semicolon()?.unwrap_or(cloned_tok);
                    let prop = Property {
                        label,
                        name: ident,
                        values,
                        end: end_tok,
                    };
                    if node_discovered {
                        self.diagnostics.push(Diagnostic::new(
                            prop.span(),
                            self.lexer.source(),
                            ErrorCode::PropertyAfterNode,
                            "Properties must be placed before nodes",
                        ))
                    }
                    items.push(NodeItem::Property(Arc::new(prop)));
                }
                TokenKind::Semicolon => {
                    self.skip_tok();
                    self.check_is_property_name(ident.span(), &ident);
                    let prop = Property::empty(ident, label, cloned_tok);
                    if node_discovered {
                        self.diagnostics.push(Diagnostic::new(
                            prop.span(),
                            self.lexer.source(),
                            ErrorCode::PropertyAfterNode,
                            "Properties must be placed before nodes",
                        ))
                    }
                    items.push(NodeItem::Property(Arc::new(prop)));
                }
                _ => {
                    return Err(Diagnostic::expected(
                        self.lexer.last_pos().as_span(),
                        self.lexer.source(),
                        &[TokenKind::Semicolon, TokenKind::Equal, TokenKind::OpenBrace],
                    ));
                }
            }
        }
        Ok(NodePayload { items, end })
    }

    /// Special function to expect a semicolon, but recover in common circumstances
    /// such as forgetting the semicolon after a closing brace ('}') char.
    fn expect_semicolon(&mut self) -> Result<Option<Token>> {
        let tok = self.lexer.peek();
        let Some(tok) = tok else {
            self.diagnostics.push(Diagnostic::expected(
                self.lexer.last_pos().as_span(),
                self.lexer.source(),
                &[TokenKind::Semicolon],
            ));
            return Ok(None);
        };
        let skip = match &tok.kind {
            TokenKind::Semicolon => {
                return Ok(self.lexer.next());
            }
            TokenKind::Slash
            | TokenKind::OpenBracket
            | TokenKind::OpenParen
            | TokenKind::Comma
            | TokenKind::OpenBrace
            | TokenKind::Ident(_)
            | TokenKind::Label(_)
            | TokenKind::String(_)
            | TokenKind::UnparsedNumber(_)
            | TokenKind::Directive(_)
            | TokenKind::Ref(_)
            | TokenKind::CloseBracket
            | TokenKind::ChevronRight
            | TokenKind::CloseBrace
            | TokenKind::Equal
            | TokenKind::CloseParen
            | TokenKind::ChevronLeft => false,
            TokenKind::Unknown(_) | TokenKind::Comment(_) => true,
        };
        let span = if skip {
            let tok = self.lexer.expect_next()?;
            self.lexer.insert_pseudo_kind(TokenKind::Semicolon);
            tok.span()
        } else {
            self.lexer.last_pos().as_char_span()
        };
        self.diagnostics.push(Diagnostic::expected(
            span,
            self.lexer.source(),
            &[TokenKind::Semicolon],
        ));
        Ok(None)
    }

    pub fn file(&mut self) -> Result<DtsFile> {
        let mut elements: Vec<Primary> = vec![];
        while self.lexer.peek().is_some() {
            elements.push(self.primary()?);
        }
        Ok(DtsFile {
            elements,
            source: self.lexer.source(),
        })
    }

    pub fn parse_reference(&mut self) -> Result<WithToken<crate::dts::ast::Reference>> {
        let token = self.lexer.expect_next()?;
        if let TokenKind::Ref(reference) = token.kind.clone() {
            Ok(self.reference(token, &reference))
        } else {
            Err(Diagnostic::expected(
                token.span(),
                token.source(),
                &[TokenKind::Ref(Reference::Simple("".to_string()))],
            ))
        }
    }

    pub fn primary(&mut self) -> Result<Primary> {
        let token = self.lexer.expect_next()?;
        match &token.kind {
            TokenKind::Directive(CompilerDirective::DTSVersionHeader) => {
                self.expect_semicolon()?;
                Ok(Primary::Directive(AnyDirective::DtsHeader(token.clone())))
            }
            TokenKind::Directive(CompilerDirective::Plugin) => {
                self.expect_semicolon()?;
                Ok(Primary::Directive(AnyDirective::Plugin(token.clone())))
            }
            TokenKind::Directive(CompilerDirective::MemReserve) => {
                let address = self.memreserve_number_u64()?;
                let length = self.memreserve_number_u64()?;
                self.expect_semicolon()?;
                Ok(Primary::Directive(AnyDirective::Memreserve(
                    Memreserve::new(address, length),
                )))
            }
            TokenKind::Ident(str) if str == "#include" => {
                let tok = self.lexer.expect_next()?;
                match tok.kind {
                    TokenKind::String(include_str) => Ok(Primary::CStyleInclude(include_str)),
                    _ => Err(Diagnostic::expected(
                        tok.span(),
                        tok.source(),
                        &[TokenKind::String("".to_string())],
                    )),
                }
            }
            TokenKind::Directive(CompilerDirective::Include) => {
                let include_token = token;
                let string_tok = self.lexer.expect_next()?;
                // let tok_span = string_tok.span();
                let path = match string_tok.kind.clone() {
                    TokenKind::String(string) => string,
                    _ => {
                        return Err(Diagnostic::expected(
                            string_tok.span(),
                            string_tok.source(),
                            &[TokenKind::String("".into())],
                        ));
                    }
                };
                if self.lexer.peek_kind() == Some(&TokenKind::Semicolon) {
                    let tok = self.lexer.expect_next()?;
                    self.diagnostics.push(Diagnostic::from_token(
                        tok,
                        ErrorCode::ParserError,
                        "Include directive must not end with a semicolon",
                    ))
                }
                Ok(Primary::Directive(AnyDirective::Include(Include {
                    include_token,
                    file_name: WithToken::new(path, string_tok.clone()),
                })))
            }
            TokenKind::Slash => {
                let root_name = WithToken::new(NodeName::simple("/"), token);
                let root_payload = self.node_payload()?;

                Ok(Primary::Root(Arc::new(Node {
                    name: root_name,
                    label: None,
                    payload: root_payload,
                })))
            }
            TokenKind::Ref(reference) => {
                let reference = self.reference(token.clone(), reference);
                let root_payload = self.node_payload()?;
                Ok(Primary::ReferencedNode(ReferencedNode {
                    reference,
                    payload: root_payload,
                }))
            }
            TokenKind::Directive(CompilerDirective::DeleteNode) => {
                let reference = self.parse_reference()?;
                self.expect_semicolon()?;
                Ok(Primary::DeletedNode(token, reference))
            }
            _ => Err(Diagnostic::expected(
                token.span(),
                token.source(),
                &[
                    TokenKind::Directive(CompilerDirective::DTSVersionHeader),
                    TokenKind::Directive(CompilerDirective::MemReserve),
                    TokenKind::Directive(CompilerDirective::Include),
                    TokenKind::Directive(CompilerDirective::DeleteNode),
                    TokenKind::Slash,
                    TokenKind::Ref(Reference::Simple("".to_string())),
                ],
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::dts::ast::{
        Cell, DtsFile, Memreserve, Node, NodeItem, NodeName, NodePayload, Path, Property,
        PropertyValue, Reference, WithToken,
    };
    use crate::dts::data::HasSource;
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::error_codes::ErrorCode;
    use crate::dts::lexer::TokenKind::{Equal, OpenBrace, Semicolon};
    use crate::dts::parser::Parser;
    use crate::dts::test::Code;
    use crate::dts::{AnyDirective, HasSpan, Position, Primary};
    use std::sync::Arc;
    use std::vec;

    #[test]
    pub fn string_properties() {
        let code = Code::new("\"\"");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::String(WithToken::new("".into(), code.token()))
        );
        let code = Code::new("\"bar\"");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::String(WithToken::new("bar".into(), code.token()))
        );
    }

    #[test]
    pub fn error_tolerant_parsing() {
        let code = Code::new(
            "\
/dts-v1/

/ {
    some_prop = <5>
}",
        );
        let (_, diagnostics) = code.parse_ok(Parser::file);
        assert_eq!(
            diagnostics,
            vec![
                Diagnostic::expected(
                    Position::new(0, 8).as_char_span(),
                    code.source(),
                    &[Semicolon],
                ),
                Diagnostic::expected(
                    Position::new(3, 19).as_char_span(),
                    code.source(),
                    &[Semicolon],
                ),
                Diagnostic::expected(Position::new(4, 1).as_span(), code.source(), &[Semicolon],),
            ]
        )
    }

    #[test]
    pub fn cell_properties() {
        let code = Code::new("<>");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Cells(code.s1("<").token(), vec![], code.s1(">").token())
        );
        let code = Code::new("<0>");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Cells(
                code.s1("<").token(),
                vec![Cell::Number(WithToken::new(0, code.s1("0").token()))],
                code.s1(">").token(),
            )
        );
        let code = Code::new("<4>");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Cells(
                code.s1("<").token(),
                vec![Cell::Number(WithToken::new(4, code.s1("4").token()))],
                code.s1(">").token(),
            )
        );
        let code = Code::new("<4 17>");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Cells(
                code.s1("<").token(),
                vec![
                    Cell::Number(WithToken::new(4, code.s1("4").token())),
                    Cell::Number(WithToken::new(17, code.s1("17").token())),
                ],
                code.s1(">").token(),
            )
        );
        let code = Code::new("<17 0xC>");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Cells(
                code.s1("<").token(),
                vec![
                    Cell::Number(WithToken::new(17, code.s1("17").token())),
                    Cell::Number(WithToken::new(0xC, code.s1("0xC").token())),
                ],
                code.s1(">").token(),
            )
        );
        let code = Code::new("<17 &label>");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Cells(
                code.s1("<").token(),
                vec![
                    Cell::Number(WithToken::new(17, code.s1("17").token())),
                    Cell::Reference(WithToken::new(
                        Reference::Label("label".into()),
                        code.s1("&label").token(),
                    )),
                ],
                code.s1(">").token(),
            )
        );
    }

    #[test]
    pub fn reference_properties() {
        let code = Code::new("&my_ref");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Reference(WithToken::new(
                Reference::Label("my_ref".into()),
                code.token(),
            ))
        );
        let code = Code::new("&{/path/to/somewhere@2000}");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::Reference(WithToken::new(
                Reference::Path(Path::new(vec![
                    NodeName::simple("path"),
                    NodeName::simple("to"),
                    NodeName::with_address("somewhere", "2000"),
                ])),
                code.token(),
            ))
        );
    }

    #[test]
    pub fn byte_strings() {
        let code = Code::new("[]");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::ByteStrings(code.s1("[").token(), vec![], code.s1("]").token())
        );
        let code = Code::new("[000012345678]");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::ByteStrings(
                code.s1("[").token(),
                vec![WithToken::new(
                    vec![0x00, 0x00, 0x12, 0x34, 0x56, 0x78],
                    code.s1("000012345678").token(),
                )],
                code.s1("]").token(),
            )
        );
        let code = Code::new("[00 00 12 34 56 78]");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::ByteStrings(
                code.s1("[").token(),
                vec![
                    WithToken::new(vec![0x00], code.s("00", 1).token()),
                    WithToken::new(vec![0x00], code.s("00", 2).token()),
                    WithToken::new(vec![0x12], code.s1("12").token()),
                    WithToken::new(vec![0x34], code.s1("34").token()),
                    WithToken::new(vec![0x56], code.s1("56").token()),
                    WithToken::new(vec![0x78], code.s1("78").token()),
                ],
                code.s1("]").token(),
            )
        );
        let code = Code::new("[AB CD]");
        assert_eq!(
            code.parse_ok_no_diagnostics(Parser::property_value),
            PropertyValue::ByteStrings(
                code.s1("[").token(),
                vec![
                    WithToken::new(vec![0xAB], code.s1("AB").token()),
                    WithToken::new(vec![0xCD], code.s1("CD").token()),
                ],
                code.s1("]").token(),
            )
        );
    }

    #[test]
    pub fn simple_file() {
        let code = Code::new("/ {};");
        let node = code.parse_ok_no_diagnostics(Parser::file);
        code.s1(";");
        assert_eq!(
            node,
            DtsFile {
                elements: vec![Primary::Root(Arc::new(Node {
                    label: None,
                    name: WithToken::new(NodeName::simple("/"), code.s1("/").token()),
                    payload: NodePayload {
                        items: vec![],
                        end: code.s1(";").token(),
                    },
                }))],
                source: code.source(),
            }
        )
    }

    #[test]
    pub fn file_with_dts_header() {
        let code = Code::new(
            "\
/dts-v1/;

/{};",
        );
        let node = code.parse_ok_no_diagnostics(Parser::file);
        assert_eq!(
            node,
            DtsFile {
                elements: vec![
                    Primary::Directive(AnyDirective::DtsHeader(code.s1("/dts-v1/").token())),
                    Primary::Root(Arc::new(Node {
                        label: None,
                        name: WithToken::new(NodeName::simple("/"), code.s("/", 3).token()),
                        payload: NodePayload {
                            items: vec![],
                            end: code.s(";", 2).token(),
                        },
                    })),
                ],
                source: code.source(),
            }
        )
    }

    #[test]
    pub fn file_with_single_simple_sub_node() {
        let code = Code::new(
            "\
/dts-v1/;

/{
    my_node: sub_node@200 {
        // my sub node
    };
};",
        );
        let node = code.parse_ok_no_diagnostics(Parser::file);
        assert_eq!(
            node,
            DtsFile {
                elements: vec![
                    Primary::Directive(AnyDirective::DtsHeader(code.s1("/dts-v1/").token())),
                    Primary::Root(Arc::new(Node {
                        label: None,
                        name: WithToken::new(NodeName::simple("/"), code.s("/", 3).token()),
                        payload: NodePayload {
                            items: vec![NodeItem::Node(Arc::new(Node {
                                label: Some(WithToken::new(
                                    "my_node".into(),
                                    code.s1("my_node:").token(),
                                )),
                                name: WithToken::new(
                                    NodeName::with_address("sub_node", "200"),
                                    code.s1("sub_node@200").token(),
                                ),
                                payload: NodePayload {
                                    items: vec![],
                                    end: code.s(";", 2).token(),
                                },
                            }))],
                            end: code.s(";", 3).token(),
                        },
                    })),
                ],
                source: code.source(),
            }
        )
    }

    #[test]
    pub fn file_with_node() {
        let code = Code::new(
            "\
    /dts-v1/;

    / {
        pic@10000000 {
            phandle = <1>;
            interrupt-controller;
            reg = <0x10000000 0x100>;
        };
    };",
        );
        let node = code.parse_ok_no_diagnostics(Parser::file);
        assert_eq!(
            node,
            DtsFile {
                elements: vec![
                    Primary::Directive(AnyDirective::DtsHeader(code.s1("/dts-v1/").token())),
                    Primary::Root(Arc::new(Node {
                        label: None,
                        name: WithToken::new(NodeName::simple("/"), code.s("/", 3).token()),
                        payload: NodePayload {
                            items: vec![NodeItem::Node(Arc::new(Node {
                                label: None,
                                name: WithToken::new(
                                    NodeName::with_address("pic", "10000000"),
                                    code.s1("pic@10000000").token(),
                                ),
                                payload: NodePayload {
                                    items: vec![
                                        NodeItem::Property(Arc::new(Property {
                                            label: None,
                                            name: WithToken::new(
                                                "phandle".into(),
                                                code.s1("phandle").token(),
                                            ),
                                            values: code
                                                .s1("<1>")
                                                .parse_ok_no_diagnostics(Parser::property_values),
                                            end: code.s(";", 2).token(),
                                        })),
                                        NodeItem::Property(Arc::new(Property {
                                            label: None,
                                            name: WithToken::new(
                                                "interrupt-controller".into(),
                                                code.s1("interrupt-controller").token(),
                                            ),
                                            values: vec![],
                                            end: code.s(";", 3).token(),
                                        })),
                                        NodeItem::Property(Arc::new(Property {
                                            label: None,
                                            name: WithToken::new(
                                                "reg".into(),
                                                code.s1("reg").token(),
                                            ),
                                            values: code
                                                .s1("<0x10000000 0x100>")
                                                .parse_ok_no_diagnostics(Parser::property_values),
                                            end: code.s(";", 4).token(),
                                        })),
                                    ],
                                    end: code.s(";", 5).token(),
                                },
                            }))],
                            end: code.s(";", 6).token(),
                        },
                    })),
                ],
                source: code.source(),
            }
        )
    }

    #[test]
    fn properties_must_be_placed_before_nodes() {
        let code = Code::new(
            "\
    /dts-v1/;

    / {
        f {
             foo;
             g {
             };
             bar;
        };
        some_prop = <0x1>;
    };",
        );
        let (_, diag) = code.parse_ok(Parser::file);
        assert_eq!(
            diag,
            vec![
                Diagnostic::new(
                    code.s1("bar;").span(),
                    code.source(),
                    ErrorCode::PropertyAfterNode,
                    "Properties must be placed before nodes"
                ),
                Diagnostic::new(
                    code.s1("some_prop = <0x1>;").span(),
                    code.source(),
                    ErrorCode::PropertyAfterNode,
                    "Properties must be placed before nodes"
                ),
            ]
        );
    }

    #[test]
    fn parses_memreserve() {
        let code = Code::new(
            "\
    /dts-v1/;
    /memreserve/ 0x10000000 0x4000;

    / {};
    ",
        );
        let node = code.parse_ok_no_diagnostics(Parser::file);
        assert_eq!(
            node,
            DtsFile {
                elements: vec![
                    Primary::Directive(AnyDirective::DtsHeader(code.s1("/dts-v1/").token())),
                    Primary::Directive(AnyDirective::Memreserve(Memreserve::new(
                        WithToken::new(0x10000000, code.s1("0x10000000").token()),
                        WithToken::new(0x4000, code.s1("0x4000").token()),
                    ))),
                    Primary::Root(Arc::new(Node {
                        label: None,
                        name: WithToken::new(NodeName::simple("/"), code.s("/", 5).token()),
                        payload: NodePayload {
                            items: vec![],
                            end: code.s(";", 3).token(),
                        },
                    })),
                ],
                source: code.source(),
            }
        );
    }

    #[test]
    fn labels() {
        // Should simply parse; we ignore these labels for now
        let _ = Code::new(
            "\
    /dts-v1/;
    /memreserve/ 0x10000000 0x4000;

    / {
        reg = reglabel: <0 sizelabel: 0x1000000>;
        prop = [ab cd ef byte4: 00 ff fe];
        str = start: \"string value\" end: ;
    };
    ",
        )
        .parse_ok_no_diagnostics(Parser::file);
    }

    #[test]
    fn expressions() {
        // Should simply parse; we ignore these expressions for now
        let _ = Code::new(
            "\
    /dts-v1/;

    / {
        some_prop = <(1 + 1) (2 || (3 - 4)) ()>;
    };
    ",
        )
        .parse_ok_no_diagnostics(Parser::file);
    }

    #[test]
    fn c_style_includes() {
        // Should simply parse; we ignore c-style includes for now
        let _ = Code::new(
            "\
    /dts-v1/;
    #include \"some_header\"

    / {};
    ",
        )
        .parse_ok_no_diagnostics(Parser::file);
    }

    #[test]
    fn error_position() {
        let code = Code::new(
            "\
    /dts-v1/;

    / {
        prop_a
        prop_b;
    };
    ",
        );
        let (res, _) = code.parse(Parser::file);

        assert_eq!(
            res,
            Err(Diagnostic::expected(
                code.s1("prop_a").end().as_span(),
                code.source(),
                &[Semicolon, Equal, OpenBrace],
            ))
        )
    }

    #[test]
    fn eof_error_position() {
        let code = Code::new(
            "\
        /dts-v1/;

        / {
        }

        ",
        );
        let (_, diag) = code.parse_ok(Parser::file);

        assert_eq!(
            diag,
            vec![Diagnostic::expected(
                code.s1("}").end().as_span(),
                code.source(),
                &[Semicolon],
            )]
        );
    }

    #[test]
    fn delete_property_syntax() {
        let code = Code::new(
            "
/ {
    node-2 {
        /delete-property/ node-2-pa;
    };
};
        ",
        );
        let primary = code.parse_ok_no_diagnostics(Parser::primary);

        assert_eq!(
            primary,
            Primary::Root(Arc::new(Node {
                label: None,
                name: WithToken::new(NodeName::simple("/"), code.s1("/").token()),
                payload: NodePayload {
                    items: vec![NodeItem::Node(Arc::new(Node {
                        label: None,
                        name: WithToken::new(NodeName::simple("node-2"), code.s1("node-2").token()),
                        payload: NodePayload {
                            end: code.s(";", 2).token(),
                            items: vec![NodeItem::DeletedProperty(
                                code.s1("/delete-property/").token(),
                                WithToken::new(
                                    "node-2-pa".to_string(),
                                    code.s1("node-2-pa").token()
                                )
                            )]
                        }
                    }))],
                    end: code.s(";", 3).token()
                }
            }))
        );
    }

    #[test]
    pub fn delete_node_primary() {
        let code = Code::new(
            "
/dts-v1/;

/delete-node/ &some_node;
/delete-node/ &{/path/to/node};
        ",
        );
        let file = code.parse_ok_no_diagnostics(Parser::file);

        assert_eq!(
            file,
            DtsFile {
                source: code.source(),
                elements: vec![
                    Primary::Directive(AnyDirective::DtsHeader(code.s1("/dts-v1/").token())),
                    Primary::DeletedNode(
                        code.s("/delete-node/", 1).token(),
                        WithToken::new(
                            Reference::Label("some_node".to_string()),
                            code.s1("&some_node").token()
                        )
                    ),
                    Primary::DeletedNode(
                        code.s("/delete-node/", 2).token(),
                        WithToken::new(
                            Reference::Path("/path/to/node".into()),
                            code.s1("&{/path/to/node}").token()
                        )
                    )
                ]
            }
        );
    }
}
