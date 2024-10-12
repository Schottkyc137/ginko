use crate::dts::ast::expression::IntConstant;
use crate::dts::ast::node::Node;
use crate::dts::ast::{ast_node, impl_from_str, Cast, CastExt};
use crate::dts::eval::property::UnquoteStrExtension;
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::{Parser, SyntaxNode, SyntaxToken};

ast_node! {
    struct File(FILE);
}

impl_from_str!(File => Parser::parse_file);

impl File {
    pub fn children(&self) -> impl Iterator<Item = FileItemKind> {
        self.0.children().filter_map(FileItemKind::cast)
    }
}

#[derive(Debug)]
pub enum FileItemKind {
    Header(Header),
    Include(Include),
    ReserveMemory(ReserveMemory),
    Node(Node),
}

impl FileItemKind {
    pub fn cast(node: SyntaxNode) -> Option<FileItemKind> {
        Some(match node.kind() {
            HEADER => FileItemKind::Header(Header::cast_unchecked(node)),
            INCLUDE_FILE => FileItemKind::Include(Include::cast_unchecked(node)),
            RESERVE_MEMORY => FileItemKind::ReserveMemory(ReserveMemory::cast_unchecked(node)),
            NODE => FileItemKind::Node(Node::cast_unchecked(node)),
            _ => return None,
        })
    }
}

ast_node! {
    struct Header(HEADER);
}

impl Header {
    pub fn token(&self) -> SyntaxToken {
        self.0.first_token().unwrap()
    }

    pub fn semicolon(&self) -> SyntaxToken {
        self.0.last_token().unwrap()
    }

    pub fn kind(&self) -> HeaderKind {
        match self.token().kind() {
            DTS_V1 => HeaderKind::DtsV1,
            PLUGIN => HeaderKind::Plugin,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum HeaderKind {
    DtsV1,
    Plugin,
}

ast_node! {
    struct Include(INCLUDE_FILE);
}

impl Include {
    pub fn target_tok(&self) -> Option<SyntaxToken> {
        self.0.last_token().filter(|tok| tok.kind() == STRING)
    }

    pub fn target(&self) -> Option<String> {
        self.0.last_token().map(|tok| tok.to_string().unquote())
    }
}

ast_node! {
    struct ReserveMemory(RESERVE_MEMORY);
}

impl ReserveMemory {
    pub fn token(&self) -> Option<SyntaxToken> {
        self.0.first_token()
    }

    pub fn semicolon(&self) -> Option<SyntaxToken> {
        self.0.last_token()
    }

    pub fn address(&self) -> IntConstant {
        self.0.children().nth(0).unwrap().cast().unwrap()
    }

    pub fn length(&self) -> IntConstant {
        self.0.children().nth(1).unwrap().cast().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::file::{File, FileItemKind, HeaderKind};
    use assert_matches::assert_matches;
    use itertools::Itertools;

    #[test]
    fn file_with_header() {
        let file = "/dts-v1/;".parse::<File>().unwrap();
        let children = file.children().collect_vec();
        assert_eq!(children.len(), 1);
        match &children[0] {
            FileItemKind::Header(header) => assert_eq!(header.kind(), HeaderKind::DtsV1),
            _ => panic!("Expected header"),
        }
        let file = "/plugin/;".parse::<File>().unwrap();
        let children = file.children().collect_vec();
        assert_eq!(children.len(), 1);
        match &children[0] {
            FileItemKind::Header(header) => assert_eq!(header.kind(), HeaderKind::Plugin),
            _ => panic!("Expected header"),
        }
    }

    #[test]
    fn file_with_mem_reserve() {
        let file = "/memreserve/ 0xABCDEF 0x123456;".parse::<File>().unwrap();
        let children = file.children().collect_vec();
        assert_eq!(children.len(), 1);
        match &children[0] {
            FileItemKind::ReserveMemory(header) => {
                assert_eq!(header.address().text(), "0xABCDEF");
                assert_eq!(header.length().text(), "0x123456");
            }
            _ => panic!("Expected header"),
        }
    }

    #[test]
    fn file_with_header_and_nodes() {
        let file = r#"
/dts-v1/;

/ {
  prop_a = [AB CD EF];
  prop_b = <0x32>;
  subnode_a {
    prop_c = "Property C";
  };
};

/ {
  #size-cells = <1>;
};
        "#
        .parse::<File>()
        .unwrap();
        let children = file.children().collect_vec();
        assert_eq!(children.len(), 3);
        assert_matches!(children[0], FileItemKind::Header(_));
        assert_matches!(children[1], FileItemKind::Node(_));
        assert_matches!(children[2], FileItemKind::Node(_));
    }

    #[test]
    fn include() {
        let file = r#"
/include/ "other_file.dtsi"
/include/ "path/to/some\\file"
        "#
        .parse::<File>()
        .unwrap();
        let children = file.children().collect_vec();
        assert_eq!(children.len(), 2);
        match &children[0] {
            FileItemKind::Include(include) => {
                assert_eq!(include.target(), Some("other_file.dtsi".to_string()))
            }
            _ => panic!("Expected include"),
        }
        match &children[1] {
            FileItemKind::Include(include) => {
                assert_eq!(include.target(), Some("path/to/some\\file".to_string()))
            }
            _ => panic!("Expected include"),
        }
    }
}
