use crate::dts::syntax::SyntaxKind;

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: SyntaxKind,
    pub value: String,
}

impl Token {
    pub fn new(kind: SyntaxKind, value: String) -> Token {
        Token { kind, value }
    }

    pub fn is_whitespace(&self) -> bool {
        self.kind == SyntaxKind::WHITESPACE
    }

    pub fn is_comment(&self) -> bool {
        self.kind == SyntaxKind::LINE_COMMENT || self.kind == SyntaxKind::BLOCK_COMMENT
    }
}
