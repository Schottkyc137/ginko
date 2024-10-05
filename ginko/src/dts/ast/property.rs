use crate::dts::ast::ast_node;
use crate::dts::expression::lex::lex;
use crate::dts::expression::SyntaxKind::*;
use crate::dts::syntax::{Parser, SyntaxToken};
use std::str::FromStr;

ast_node! {
    struct ByteString(BYTE_STRING);
}

ast_node! {
    struct ByteChunk(BYTE_CHUNK);
}

impl ByteChunk {
    pub fn text(&self) -> String {
        self.0.first_token().unwrap().text().to_owned()
    }
}

impl ByteString {
    pub fn r_brak(&self) -> SyntaxToken {
        self.0.last_token().unwrap()
    }

    pub fn l_brak(&self) -> SyntaxToken {
        self.0.first_token().unwrap()
    }

    pub fn contents(&self) -> impl Iterator<Item = ByteChunk> {
        self.0.children().filter_map(ByteChunk::cast)
    }
}

impl FromStr for ByteString {
    type Err = Vec<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ast, errors) = Parser::new(lex(s).into_iter()).parse(Parser::parse_byte_strings);
        if errors.is_empty() {
            Ok(ByteString::cast(ast).unwrap())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::property::ByteString;
    use rowan::{TextRange, TextSize};

    #[test]
    fn check_empty_byte_string() {
        let byte_string = "[]".parse::<ByteString>().unwrap();
        assert_eq!(
            byte_string.l_brak().text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(1))
        );
        assert_eq!(
            byte_string.r_brak().text_range(),
            TextRange::new(TextSize::new(1), TextSize::new(2))
        );
    }

    #[test]
    fn single_byte_string() {
        let byte_string = "[000012345678]".parse::<ByteString>().unwrap();
        assert_eq!(
            byte_string.l_brak().text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(1))
        );
        assert_eq!(
            byte_string.r_brak().text_range(),
            TextRange::new(TextSize::new(13), TextSize::new(14))
        );
        assert_eq!(byte_string.contents().count(), 1);
        let byte_string = "[AB]".parse::<ByteString>().unwrap();
        assert_eq!(byte_string.contents().count(), 1);
    }

    #[test]
    fn multiple_elements_in_byte_string() {
        let byte_string = "[00 00 12 34 56 78]".parse::<ByteString>().unwrap();
        assert_eq!(byte_string.contents().count(), 6);
        let byte_string = "[AB CD]".parse::<ByteString>().unwrap();
        assert_eq!(byte_string.contents().count(), 2);
    }
}
