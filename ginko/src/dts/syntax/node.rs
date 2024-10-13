use crate::dts::lex::token::Token;
use crate::dts::syntax::multipeek::MultiPeek;
use crate::dts::syntax::parser::Parser;
use crate::dts::syntax::SyntaxKind::*;

impl<M> Parser<M>
where
    M: MultiPeek<Token> + Iterator<Item = Token>,
{
    pub fn property_or_node_name(&mut self) {
        self.skip_ws();
        self.start_node(NAME);
        while matches!(
            self.peek_kind_direct(),
            Some(
                IDENT
                    | NUMBER
                    | COMMA
                    | DOT
                    | UNDERSCORE
                    | PLUS
                    | MINUS
                    | QUESTION_MARK
                    | POUND
                    | AT
            )
        ) {
            self.bump();
        }
        self.finish_node();
    }

    pub fn parse_node_body(&mut self) {
        match self.peek_kind() {
            Some(L_BRACE) => {
                self.start_node(NODE_BODY);
                self.bump();
            }
            Some(_) => {
                self.error_token("Expected '{'");
                return;
            }
            None => {
                self.finish_node();
                return;
            }
        }
        loop {
            match self.peek_kind() {
                Some(R_BRACE) => {
                    self.bump();
                    break;
                }
                Some(_) => self.parse_property_or_node(),
                None => {
                    self.unexpected_eof();
                    break;
                }
            }
        }
        self.finish_node();
    }

    #[allow(unused)]
    pub fn parse_node(&mut self) {
        self.start_node(NODE);
        if let Some(OMIT_IF_NO_REF) = self.peek_kind() {
            self.bump_into_node(DECORATION)
        }
        self.property_or_node_name();
        self.parse_node_body();
        self.expect(SEMICOLON);
        self.finish_node();
    }

    pub fn parse_property_or_node(&mut self) {
        let checkpoint = self.checkpoint();
        self.parse_optional_label();
        match self.peek_kind() {
            Some(DELETE_NODE) => {
                self.start_node(DELETE_SPEC);
                self.bump();
                self.property_or_node_name();
                self.expect(SEMICOLON);
                self.finish_node();
                return;
            }
            Some(DELETE_PROPERTY) => {
                self.start_node(DELETE_SPEC);
                self.bump();
                self.property_or_node_name();
                self.expect(SEMICOLON);
                self.finish_node();
                return;
            }
            Some(OMIT_IF_NO_REF) => {
                self.bump_into_node(DECORATION);
            }
            _ => {}
        };
        if matches!(
            self.peek_kind(),
            Some(IDENT | NUMBER | COMMA | DOT | UNDERSCORE | PLUS | MINUS | QUESTION_MARK | POUND)
        ) {
            self.property_or_node_name();
            match self.peek_kind() {
                Some(SEMICOLON) => {
                    self.start_node_at(checkpoint, PROPERTY);
                    self.bump();
                    self.finish_node();
                }
                Some(EQ) => {
                    self.start_node_at(checkpoint, PROPERTY);
                    self.bump();

                    self.parse_property_list();
                    self.expect(SEMICOLON);
                    self.finish_node();
                }
                Some(L_BRACE) => {
                    self.start_node_at(checkpoint, NODE);
                    self.parse_node_body();
                    self.expect(SEMICOLON);
                    self.finish_node();
                }
                Some(_) => {
                    self.start_node_at(checkpoint, ERROR);
                    self.error_token("Expected node or property");
                    self.finish_node();
                }
                None => {
                    self.start_node_at(checkpoint, ERROR);
                    self.unexpected_eof();
                    self.finish_node();
                }
            }
        } else if self.peek_kind().is_none() {
            self.start_node_at(checkpoint, ERROR);
            self.unexpected_eof();
            self.finish_node();
        } else {
            self.start_node_at(checkpoint, ERROR);
            self.error_token("Expected node or property");
            self.finish_node();
        }
    }
}

#[cfg(test)]
mod test {
    use crate::dts::syntax::testing::check_generic;
    use crate::dts::syntax::Parser;

    fn check_property_or_node(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_property_or_node)
    }

    #[test]
    fn simple_property() {
        check_property_or_node(
            "prop = <12>;",
            r#"
PROPERTY
  NAME
    IDENT "prop"
  WHITESPACE " "
  EQ "="
  WHITESPACE " "
  PROPERTY_LIST
    PROP_VALUE
      CELL
        CELL_INNER
          L_CHEV "<"
          INT
            NUMBER "12"
          R_CHEV ">"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn property_with_label() {
        check_property_or_node(
            "labeled: prop;",
            r#"
PROPERTY
  LABEL
    IDENT "labeled"
    COLON ":"
  WHITESPACE " "
  NAME
    IDENT "prop"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn empty_property() {
        check_property_or_node(
            "prop;",
            r#"
PROPERTY
  NAME
    IDENT "prop"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn deleted_property() {
        check_property_or_node(
            "/delete-property/ prop;",
            r#"
DELETE_SPEC
  DELETE_PROPERTY "/delete-property/"
  WHITESPACE " "
  NAME
    IDENT "prop"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn deleted_node() {
        check_property_or_node(
            "/delete-node/ prop;",
            r#"
DELETE_SPEC
  DELETE_NODE "/delete-node/"
  WHITESPACE " "
  NAME
    IDENT "prop"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn empty_node() {
        check_property_or_node(
            "empty {};",
            r#"
NODE
  NAME
    IDENT "empty"
  WHITESPACE " "
  NODE_BODY
    L_BRACE "{"
    R_BRACE "}"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn omit_if_no_ref_node() {
        check_property_or_node(
            "/omit-if-no-ref/ empty {};",
            r#"
NODE
  DECORATION
    OMIT_IF_NO_REF "/omit-if-no-ref/"
  WHITESPACE " "
  NAME
    IDENT "empty"
  WHITESPACE " "
  NODE_BODY
    L_BRACE "{"
    R_BRACE "}"
  SEMICOLON ";"
"#,
        );
    }

    #[test]
    fn node_with_empty_property() {
        check_property_or_node(
            "empty { some_prop; };",
            r#"
NODE
  NAME
    IDENT "empty"
  WHITESPACE " "
  NODE_BODY
    L_BRACE "{"
    WHITESPACE " "
    PROPERTY
      NAME
        IDENT "some_prop"
      SEMICOLON ";"
    WHITESPACE " "
    R_BRACE "}"
  SEMICOLON ";"
"#,
        );
    }

    fn check_property_name(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::property_or_node_name)
    }

    #[test]
    fn check_property_names() {
        check_property_name(
            "node",
            r#"
NAME
  IDENT "node"
"#,
        );
        check_property_name(
            "#size-cells",
            r##"
NAME
  POUND "#"
  IDENT "size"
  MINUS "-"
  IDENT "cells"
"##,
        );
        check_property_name(
            "fsbl,my_node#s",
            r##"
NAME
  IDENT "fsbl"
  COMMA ","
  IDENT "my_node"
  POUND "#"
  IDENT "s"
"##,
        );
    }

    fn check_node_body(expression: &str, expected: &str) {
        check_generic(expression, expected, Parser::parse_node_body)
    }

    #[test]
    fn empty_node_body() {
        check_node_body(
            "{}",
            r#"
NODE_BODY
  L_BRACE "{"
  R_BRACE "}"
"#,
        )
    }

    #[test]
    fn node_body_with_sub_node() {
        check_node_body(
            "\
{
  sub_node {
    empty_prop;
  };
}",
            r#"
NODE_BODY
  L_BRACE "{"
  WHITESPACE "\n  "
  NODE
    NAME
      IDENT "sub_node"
    WHITESPACE " "
    NODE_BODY
      L_BRACE "{"
      WHITESPACE "\n    "
      PROPERTY
        NAME
          IDENT "empty_prop"
        SEMICOLON ";"
      WHITESPACE "\n  "
      R_BRACE "}"
    SEMICOLON ";"
  WHITESPACE "\n"
  R_BRACE "}"
"#,
        )
    }
}
