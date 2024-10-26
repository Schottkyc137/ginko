use crate::dts::ast::label::Label;
use crate::dts::ast::property::PropertyList;
use crate::dts::ast::{ast_node, impl_from_str};
use crate::dts::syntax::Parser;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::{Lang, SyntaxToken};
use rowan::ast::AstNode;
use rowan::Language;
use std::error::Error;
use std::fmt::{Display, Formatter};

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

impl Display for IllegalNodeName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IllegalNodeName::IllegalChar(str, pos) => {
                write!(
                    f,
                    "Name contains illegal character '{}'",
                    str.chars().nth(*pos).unwrap()
                )
            }
        }
    }
}

impl Error for IllegalNodeName {}

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

impl rowan::ast::AstNode for NodeOrProperty {
    type Language = Lang;

    fn can_cast(kind: <Self::Language as Language>::Kind) -> bool
    where
        Self: Sized,
    {
        matches!(kind, NODE | PROPERTY | DELETE_SPEC)
    }

    fn cast(node: rowan::SyntaxNode<Self::Language>) -> Option<Self>
    where
        Self: Sized,
    {
        Some(match node.kind() {
            NODE => NodeOrProperty::Node(Node::cast_unchecked(node)),
            PROPERTY => NodeOrProperty::Property(Property::cast_unchecked(node)),
            DELETE_SPEC => NodeOrProperty::DeleteSpec(DeleteSpec::cast_unchecked(node)),
            _ => return None,
        })
    }

    fn syntax(&self) -> &rowan::SyntaxNode<Self::Language> {
        match self {
            NodeOrProperty::Node(node) => node.syntax(),
            NodeOrProperty::Property(property) => property.syntax(),
            NodeOrProperty::DeleteSpec(delete_spec) => delete_spec.syntax(),
        }
    }
}

ast_node! {
    struct Node(NODE);
}

// TODO: this panics if one parses a valid property
impl_from_str!(Node => Parser::parse_property_or_node);

impl Node {
    pub fn decoration(&self) -> Option<Decoration> {
        self.0.children().filter_map(Decoration::cast).next()
    }

    pub fn label(&self) -> Option<Label> {
        self.0.children().filter_map(Label::cast).next()
    }

    // TODO: could also be referenced node
    pub fn name(&self) -> Name {
        self.0.children().filter_map(Name::cast).next().unwrap()
    }

    pub fn body(&self) -> NodeBody {
        self.0.children().filter_map(NodeBody::cast).next().unwrap()
    }

    pub fn semicolon(&self) -> Option<SyntaxToken> {
        self.0.last_token().filter(|tok| tok.kind() == SEMICOLON)
    }
}

ast_node! {
    struct Property(PROPERTY);
}

// TODO: this panics if one parses a valid property
impl_from_str!(Property => Parser::parse_property_or_node);

impl Property {
    pub fn decoration(&self) -> Option<Decoration> {
        self.0.children().filter_map(Decoration::cast).next()
    }

    pub fn label(&self) -> Option<Label> {
        self.0.children().filter_map(Label::cast).next()
    }

    pub fn name(&self) -> Name {
        self.0.children().filter_map(Name::cast).next().unwrap()
    }

    pub fn value(&self) -> Option<PropertyList> {
        self.0.children().filter_map(PropertyList::cast).next()
    }

    pub fn semicolon(&self) -> Option<SyntaxToken> {
        self.0.last_token().filter(|tok| tok.kind() == SEMICOLON)
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
        assert!(node.decoration().is_none());
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
        assert!(node.decoration().is_none());
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
