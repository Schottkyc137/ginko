use crate::dts::ast::{ast_node, impl_from_str, Cast};
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::SyntaxToken;

ast_node! { struct Name(NAME); }

impl Name {
    pub fn text(&self) -> String {
        self.0.text().to_string()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum IllegalNodeName {
    IllegalChar(String, usize),
}

impl Name {
    pub fn node_name(&self) -> Result<String, IllegalNodeName> {
        let text = self.text();
        match text.find(
            |ch| !matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | ',' | '.' | '_' | '+' | '-'),
        ) {
            Some(pos) => Err(IllegalNodeName::IllegalChar(text, pos)),
            None => Ok(text),
        }
    }

    pub fn property_name(&self) -> Result<String, IllegalNodeName> {
        let text = self.text();
        match text.find(
            |ch| !matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | ',' | '.' | '_' | '+' | '-' | '?' | '#'),
        ) {
            Some(pos) => Err(IllegalNodeName::IllegalChar(text, pos)),
            None => Ok(text),
        }
    }
}

ast_node! {
    struct NodeBody(NODE_BODY);
}

impl NodeBody {
    pub fn l_brace(&self) -> SyntaxToken {
        self.0.first_token().unwrap()
    }

    pub fn r_brace(&self) -> SyntaxToken {
        self.0.last_token().unwrap()
    }

    pub fn children(&self) -> impl Iterator<Item = NodeOrProperty> {
        self.0.children().flat_map(NodeOrProperty::cast)
    }
}

ast_node! {
    struct NodeOrProperty(NODE | PROPERTY | DELETE_SPEC);
}

#[derive(Debug)]
pub enum NodeOrPropertyKind {
    Node(Node),
    Property(Property),
    DeleteSpec(DeleteSpec),
}

impl NodeOrProperty {
    pub fn kind(&self) -> NodeOrPropertyKind {
        match self.0.kind() {
            NODE => NodeOrPropertyKind::Node(Node::cast_unchecked(self.0.clone())),
            PROPERTY => NodeOrPropertyKind::Property(Property::cast_unchecked(self.0.clone())),
            DELETE_SPEC => {
                NodeOrPropertyKind::DeleteSpec(DeleteSpec::cast_unchecked(self.0.clone()))
            }
            _ => unreachable!(),
        }
    }
}

ast_node! {
    struct Node(NODE);
}

impl_from_str!(Node => Parser::parse_property_or_node);

ast_node! {
    struct Property(PROPERTY);
}

ast_node! {
    struct DeleteSpec(DELETE_SPEC);
}

impl Node {
    pub fn decoration(&self) -> Decoration {
        Decoration::cast(self.0.children().nth(0).unwrap()).unwrap()
    }

    // TODO: could also be referenced node
    pub fn name(&self) -> Name {
        Name::cast(self.0.children().nth(1).unwrap()).unwrap()
    }

    pub fn body(&self) -> NodeBody {
        NodeBody::cast(self.0.children().nth(2).unwrap()).unwrap()
    }

    pub fn semicolon(&self) -> SyntaxToken {
        self.0.last_token().unwrap()
    }
}

ast_node! {
    struct Decoration(DECORATION);
}

impl Decoration {
    pub fn token(&self) -> Option<SyntaxToken> {
        self.0.first_token()
    }

    pub fn omit_if_no_ref(&self) -> bool {
        self.token().is_some_and(|tok| tok.kind() == OMIT_IF_NO_REF)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::node::{Node, NodeOrPropertyKind};
    use assert_matches::assert_matches;
    use itertools::Itertools;

    #[test]
    fn check_empty_node() {
        let node = "name {};".parse::<Node>().unwrap();
        assert!(node.decoration().token().is_none());
        assert_eq!(node.name().node_name().unwrap(), "name");
        assert_eq!(node.body().children().count(), 0);
    }

    #[test]
    fn check_node_with_single_element() {
        let node = "name {
  prop_a = <32>;
  sub_node {
    prop_b = [ABCD];
  };
};"
        .parse::<Node>()
        .unwrap();
        assert!(node.decoration().token().is_none());
        assert_eq!(node.name().node_name().unwrap(), "name");
        let children = node.body().children().collect_vec();
        assert_eq!(children.len(), 2);
        assert_matches!(children[0].kind(), NodeOrPropertyKind::Property(_));
        match children[1].kind() {
            NodeOrPropertyKind::Node(node) => {
                assert_eq!(node.name().node_name().unwrap(), "sub_node");
                assert_eq!(node.body().children().count(), 1);
            }
            _ => panic!("Expected sub node"),
        }
    }
}
