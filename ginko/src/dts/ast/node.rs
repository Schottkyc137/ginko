use crate::dts::ast::property::PropertyList;
use crate::dts::ast::{ast_node, impl_from_str, Cast};
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::SyntaxToken;
use crate::dts::syntax::{Parser, SyntaxNode};

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
    // TODO: This needs to be revisited
    pub fn node_name(&self) -> Result<String, IllegalNodeName> {
        let text = self.text();
        if text == "/" {
            return Ok(text);
        }
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

#[derive(Debug)]
pub enum NodeOrProperty {
    Node(Node),
    Property(Property),
    DeleteSpec(DeleteSpec),
}

impl Cast for NodeOrProperty {
    fn cast(node: SyntaxNode) -> Option<Self> {
        Some(match node.kind() {
            NODE => NodeOrProperty::Node(Node::cast_unchecked(node)),
            PROPERTY => NodeOrProperty::Property(Property::cast_unchecked(node)),
            DELETE_SPEC => NodeOrProperty::DeleteSpec(DeleteSpec::cast_unchecked(node)),
            _ => return None,
        })
    }
}

ast_node! {
    struct Node(NODE);
}

// TODO: this panics if one parses a valid property
impl_from_str!(Node => Parser::parse_property_or_node);

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
    struct Property(PROPERTY);
}

// TODO: this panics if one parses a valid property
impl_from_str!(Property => Parser::parse_property_or_node);

impl Property {
    pub fn decoration(&self) -> Decoration {
        Decoration::cast(self.0.children().nth(0).unwrap()).unwrap()
    }

    pub fn name(&self) -> Name {
        Name::cast(self.0.children().nth(1).unwrap()).unwrap()
    }

    pub fn value(&self) -> Option<PropertyList> {
        self.0.children().nth(2).and_then(PropertyList::cast)
    }

    pub fn semicolon(&self) -> SyntaxToken {
        self.0.last_token().unwrap()
    }
}

ast_node! {
    struct DeleteSpec(DELETE_SPEC);
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
    use crate::dts::ast::node::{Node, NodeOrProperty};
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
        assert_matches!(&children[0], NodeOrProperty::Property(_));
        match &children[1] {
            NodeOrProperty::Node(node) => {
                assert_eq!(node.name().node_name().unwrap(), "sub_node");
                assert_eq!(node.body().children().count(), 1);
            }
            _ => panic!("Expected sub node"),
        }
    }
}
