use crate::dts::syntax::SyntaxKind;

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
}
