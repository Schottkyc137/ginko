use crate::dts::ast::Cast;
use crate::dts::ast::{ast_node, impl_from_str};
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::{Parser, SyntaxToken};

ast_node! {
    struct Label(LABEL);
}
impl_from_str!(Label => Parser::parse_optional_label);

impl Label {
    pub fn ident_tok(&self) -> SyntaxToken {
        self.0.first_token().unwrap()
    }

    pub fn ident(&self) -> String {
        self.ident_tok().to_string()
    }

    pub fn colon_tok(&self) -> Option<SyntaxToken> {
        self.0.last_token()
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::label::Label;
    use line_index::TextSize;
    use rowan::TextRange;

    #[test]
    fn simple_label() {
        let label = "label:".parse::<Label>().unwrap();
        assert_eq!(label.ident(), "label".to_string());
        assert_eq!(
            label.ident_tok().text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(5))
        );
        assert_eq!(
            label.colon_tok().unwrap().text_range(),
            TextRange::new(TextSize::new(5), TextSize::new(6))
        );
    }
}
