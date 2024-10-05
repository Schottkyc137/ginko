use crate::dts::expression::Lang;

mod cell;
pub mod expression;
mod file;
pub mod node;
pub mod parser;
mod property;

pub use parser::Parser;

pub type SyntaxNode = rowan::SyntaxNode<Lang>;
pub type SyntaxToken = rowan::SyntaxToken<Lang>;
pub type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

#[cfg(test)]
mod testing {
    use crate::dts::expression::lex::lex;
    use crate::dts::expression::token::Token;
    use crate::dts::expression::SyntaxKind;
    use crate::dts::syntax::{Parser, SyntaxElement};
    use std::vec::IntoIter;

    fn str(element: SyntaxElement) -> String {
        let mut buffer: String = String::new();
        _str(0, &mut buffer, element);
        buffer
    }

    fn _str(indent: usize, buffer: &mut String, element: SyntaxElement) {
        let kind: SyntaxKind = element.kind();
        buffer.push_str(&" ".repeat(indent));
        match element {
            SyntaxElement::Node(node) => {
                buffer.push_str(&format!("{:?}\n", kind));
                for child in node.children_with_tokens() {
                    _str(indent + 2, buffer, child);
                }
            }

            SyntaxElement::Token(token) => {
                buffer.push_str(&format!("{:?} {:?}\n", kind, token.text()))
            }
        }
    }

    pub fn check_generic(
        expression: &str,
        expected: &str,
        parse_fn: impl FnOnce(&mut Parser<IntoIter<Token>>),
    ) {
        let (ast, errors) = Parser::new(lex(expression).into_iter()).parse(parse_fn);
        assert!(errors.is_empty(), "Got errors {:?}", errors);
        let ast_str = str(ast.into());
        let ast_str_trimmed = ast_str.trim();
        assert_eq!(ast_str_trimmed, expected.trim());
    }
}
