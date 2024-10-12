use crate::dts::lex::token::Token;
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;

impl<I: Iterator<Item = Token>> Parser<I> {
    pub fn parse_file(&mut self) {
        self.start_node(FILE);
        loop {
            match self.peek_kind() {
                Some(DTS_V1) => {
                    self.start_node(HEADER);
                    self.bump();
                    self.expect(SEMICOLON);
                    self.finish_node();
                }
                Some(PLUGIN) => {
                    self.start_node(HEADER);
                    self.bump();
                    self.expect(SEMICOLON);
                    self.finish_node();
                }
                Some(INCLUDE) => {
                    self.start_node(INCLUDE_FILE);
                    self.bump();
                    self.expect(STRING);
                    self.finish_node();
                }
                Some(MEM_RESERVE) => self.parse_mem_reserve(),
                Some(OMIT_IF_NO_REF) => self.parse_property_or_node(),
                Some(SLASH) => {
                    self.start_node(NODE);
                    self.start_node(DECORATION);
                    self.finish_node();
                    self.bump_into_node(NAME);
                    self.parse_node_body();
                    self.expect(SEMICOLON);
                    self.finish_node();
                }
                Some(AMP) => {
                    self.start_node(NODE);
                    self.parse_reference();
                    self.parse_node_body();
                    self.expect(SEMICOLON);
                    self.finish_node();
                }
                Some(_) => {
                    self.error_token("Expected a primary construct");
                }
                None => break,
            }
        }
        // Skip trailing whitespaces
        self.skip_ws();
        self.finish_node();
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::syntax::testing::check_generic;
    use crate::dts::syntax::Parser;

    fn check_file(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_file)
    }

    #[test]
    fn empty_file() {
        check_file("", "FILE")
    }

    #[test]
    fn file_with_headers() {
        check_file(
            "/dts-v1/;",
            r#"
FILE
  HEADER
    DTS_V1 "/dts-v1/"
    SEMICOLON ";"
"#,
        );
        check_file(
            "/plugin/;",
            r#"
FILE
  HEADER
    PLUGIN "/plugin/"
    SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn include_directive() {
        check_file(
            "/include/ \"some_file\"",
            r#"
FILE
  INCLUDE_FILE
    INCLUDE "/include/"
    WHITESPACE " "
    STRING "\"some_file\""
"#,
        );
    }

    #[test]
    fn root_node() {
        check_file(
            "/ {};",
            r#"
FILE
  NODE
    DECORATION
    NAME
      SLASH "/"
    WHITESPACE " "
    NODE_BODY
      L_BRACE "{"
      R_BRACE "}"
    SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn referenced_node() {
        check_file(
            "&some_node {};",
            r#"
FILE
  NODE
    REF
      AMP "&"
      IDENT "some_node"
    WHITESPACE " "
    NODE_BODY
      L_BRACE "{"
      R_BRACE "}"
    SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn root_with_header() {
        check_file(
            "\
/dts-v1/;
/ {};",
            r#"
FILE
  HEADER
    DTS_V1 "/dts-v1/"
    SEMICOLON ";"
  WHITESPACE "\n"
  NODE
    DECORATION
    NAME
      SLASH "/"
    WHITESPACE " "
    NODE_BODY
      L_BRACE "{"
      R_BRACE "}"
    SEMICOLON ";"
"#,
        );
    }
}
