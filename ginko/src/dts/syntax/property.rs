use crate::dts::expression::token::Token;
use crate::dts::expression::SyntaxKind::*;
use crate::dts::syntax::Parser;

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_byte_strings(&mut self) {
        assert_eq!(self.peek_kind(), Some(L_BRAK));
        self.start_node(BYTE_STRING);
        self.bump();
        loop {
            match self.peek_kind() {
                Some(R_BRAK) => {
                    self.bump();
                    break;
                }
                Some(NUMBER | IDENT) => self.bump_into_node(BYTE_CHUNK),
                Some(_) => self.error_token("Expected hexadecimal value".to_string()),
                None => self.eof_error(),
            }
        }
        self.finish_node();
    }

    pub fn parse_property_value(&mut self) {
        self.start_node(PROP_VALUE);
        if self.peek_kind() == Some(BITS) {
            self.start_node(BITS_SPEC);
            self.bump();
            match self.peek_kind() {
                Some(NUMBER) => self.bump_into_node(INT),
                Some(_) => self.error_token("Expected number of bits"),
                None => {
                    self.eof_error();
                    self.finish_node();
                    return;
                }
            }
            self.finish_node()
        }
        match self.peek_kind() {
            Some(STRING) => self.bump_into_node(STRING_PROP),
            Some(L_CHEV) => self.parse_cell(),
            Some(AMP) => self.parse_reference(),
            Some(L_BRAK) => self.parse_byte_strings(),
            Some(_) => self.error_token("Expected string, cell, reference or bytes".to_string()),
            _ => self.eof_error(),
        }
        self.finish_node();
    }

    pub fn parse_property_list(&mut self) {
        self.skip_ws();
        self.start_node(PROPERTY_LIST);
        loop {
            self.parse_property_value();
            if self.peek_kind() == Some(COMMA) {
                self.bump();
            } else {
                break;
            }
        }
        self.finish_node();
    }
}

#[cfg(test)]
mod test {
    use crate::dts::syntax::testing::check_generic;
    use crate::dts::syntax::Parser;

    fn check_byte_string(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_byte_strings)
    }

    #[test]
    fn empty_byte_string() {
        check_byte_string(
            "[]",
            r#"
BYTE_STRING
  L_BRAK "["
  R_BRAK "]"
"#,
        );
        check_byte_string(
            "[  ]",
            r#"
BYTE_STRING
  L_BRAK "["
  WHITESPACE "  "
  R_BRAK "]"
"#,
        );
    }

    #[test]
    fn single_byte_string() {
        check_byte_string(
            "[000012345678]",
            r#"
BYTE_STRING
  L_BRAK "["
  BYTE_CHUNK
    NUMBER "000012345678"
  R_BRAK "]"
"#,
        );
        check_byte_string(
            "[AB]",
            r#"
BYTE_STRING
  L_BRAK "["
  BYTE_CHUNK
    IDENT "AB"
  R_BRAK "]"
"#,
        );
    }

    #[test]
    fn byte_strings_with_whitespace() {
        check_byte_string(
            "[00 00 12 34 56 78]",
            r#"
BYTE_STRING
  L_BRAK "["
  BYTE_CHUNK
    NUMBER "00"
  WHITESPACE " "
  BYTE_CHUNK
    NUMBER "00"
  WHITESPACE " "
  BYTE_CHUNK
    NUMBER "12"
  WHITESPACE " "
  BYTE_CHUNK
    NUMBER "34"
  WHITESPACE " "
  BYTE_CHUNK
    NUMBER "56"
  WHITESPACE " "
  BYTE_CHUNK
    NUMBER "78"
  R_BRAK "]"
"#,
        );
    }

    #[test]
    fn non_numbers_as_byte_strings() {
        check_byte_string(
            "[AB CD]",
            r#"
BYTE_STRING
  L_BRAK "["
  BYTE_CHUNK
    IDENT "AB"
  WHITESPACE " "
  BYTE_CHUNK
    IDENT "CD"
  R_BRAK "]"
"#,
        );
    }

    fn check_property_value(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_property_value)
    }

    #[test]
    fn simple_property_value() {
        check_property_value(
            r#""Hello, World!""#,
            r#"
PROP_VALUE
  STRING_PROP
    STRING "\"Hello, World!\""
"#,
        );
        check_property_value(
            "[AB]",
            r#"
PROP_VALUE
  BYTE_STRING
    L_BRAK "["
    BYTE_CHUNK
      IDENT "AB"
    R_BRAK "]"
"#,
        );

        check_property_value(
            "<32>",
            r#"
PROP_VALUE
  CELL
    L_CHEV "<"
    INT
      NUMBER "32"
    R_CHEV ">"
"#,
        );

        check_property_value(
            "&some_label",
            r#"
PROP_VALUE
  REFERENCE
    AMP "&"
    IDENT "some_label"
"#,
        );
    }

    #[test]
    fn property_value_with_bits() {
        check_property_value(
            "/bits/ 8 <27>",
            r#"
PROP_VALUE
  BITS_SPEC
    BITS "/bits/"
    WHITESPACE " "
    INT
      NUMBER "8"
  WHITESPACE " "
  CELL
    L_CHEV "<"
    INT
      NUMBER "27"
    R_CHEV ">"
"#,
        );
    }

    fn check_property_list(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_property_list)
    }

    #[test]
    fn multiple_properties() {
        check_property_list(
            "<23>, <47>",
            r#"
PROPERTY_LIST
  PROP_VALUE
    CELL
      L_CHEV "<"
      INT
        NUMBER "23"
      R_CHEV ">"
  COMMA ","
  PROP_VALUE
    WHITESPACE " "
    CELL
      L_CHEV "<"
      INT
        NUMBER "47"
      R_CHEV ">"
"#,
        );

        check_property_list(
            r#"&label, "string", <47>"#,
            r#"
PROPERTY_LIST
  PROP_VALUE
    REFERENCE
      AMP "&"
      IDENT "label"
  COMMA ","
  PROP_VALUE
    WHITESPACE " "
    STRING_PROP
      STRING "\"string\""
  COMMA ","
  PROP_VALUE
    WHITESPACE " "
    CELL
      L_CHEV "<"
      INT
        NUMBER "47"
      R_CHEV ">"
"#,
        );
    }
}
