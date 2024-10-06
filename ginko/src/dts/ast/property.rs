use crate::dts::ast::cell::{Cell, Reference};
use crate::dts::ast::{ast_node, impl_from_str, Cast};
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
        let (ast, errors) = Parser::new(lex(s).into_iter()).parse(Parser::parse_byte_string);
        if errors.is_empty() {
            Ok(ByteString::cast(ast).unwrap())
        } else {
            Err(errors)
        }
    }
}

ast_node! {
    struct PropertyValue(PROP_VALUE);
}

impl PropertyValue {
    pub fn bits(&self) -> Option<BitsSpec> {
        self.0.first_child().and_then(BitsSpec::cast)
    }

    pub fn kind(&self) -> PropertyValueKind {
        let node = if self.bits().is_some() {
            self.0.children().nth(1)
        } else {
            self.0.children().nth(0)
        }
        .unwrap();
        match node.kind() {
            STRING_PROP => PropertyValueKind::String(StringProperty::cast(node).unwrap()),
            CELL => PropertyValueKind::Cell(Cell::cast(node).unwrap()),
            REFERENCE => PropertyValueKind::Reference(Reference::cast(node).unwrap()),
            BYTE_STRING => PropertyValueKind::ByteString(ByteString::cast(node).unwrap()),
            _ => unreachable!(),
        }
    }
}

impl_from_str!(PropertyValue => Parser::parse_property_value);

#[derive(Debug)]
pub enum PropertyValueKind {
    String(StringProperty),
    Cell(Cell),
    Reference(Reference),
    ByteString(ByteString),
}

ast_node! {
    terminal struct StringProperty(STRING_PROP);
}

ast_node! {
    struct BitsSpec(BITS_SPEC);
}

ast_node! {
    struct PropertyList(PROPERTY_LIST);
}
impl_from_str!(PropertyList => Parser::parse_property_list);

impl PropertyList {
    pub fn items(&self) -> impl Iterator<Item = PropertyValue> {
        self.0.children().filter_map(PropertyValue::cast)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::property::{ByteString, PropertyList, PropertyValue, PropertyValueKind};
    use assert_matches::assert_matches;
    use itertools::Itertools;
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

    #[test]
    fn property_value_with_bits() {
        let byte_string = "/bits/ 8 <0x2F>".parse::<PropertyValue>().unwrap();
        assert!(byte_string.bits().is_some());
        assert_matches!(byte_string.kind(), PropertyValueKind::Cell(_))
    }

    #[test]
    fn single_property_values() {
        let byte_string = r#""Hello, World!""#.parse::<PropertyValue>().unwrap();
        assert!(byte_string.bits().is_none());
        assert_matches!(byte_string.kind(), PropertyValueKind::String(_));
        let byte_string = "<17 18>".parse::<PropertyValue>().unwrap();
        assert!(byte_string.bits().is_none());
        assert_matches!(byte_string.kind(), PropertyValueKind::Cell(_));
        let byte_string = "&other_node".parse::<PropertyValue>().unwrap();
        assert!(byte_string.bits().is_none());
        assert_matches!(byte_string.kind(), PropertyValueKind::Reference(_));
        let byte_string = "[ABCD]".parse::<PropertyValue>().unwrap();
        assert!(byte_string.bits().is_none());
        assert_matches!(byte_string.kind(), PropertyValueKind::ByteString(_))
    }

    #[test]
    fn property_list_single_element() {
        let byte_string = r#""Hello, World!""#.parse::<PropertyList>().unwrap();
        assert_eq!(byte_string.items().count(), 1)
    }

    #[test]
    fn property_list_several_elements() {
        let byte_string = r#"[AB], <27>"#.parse::<PropertyList>().unwrap();
        let elements = byte_string.items().collect_vec();
        assert_eq!(elements.len(), 2);
        assert_matches!(elements[0].kind(), PropertyValueKind::ByteString(_));
        assert_matches!(elements[1].kind(), PropertyValueKind::Cell(_));
    }
}
