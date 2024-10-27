use crate::dts::ast::expression::{IntConstant, ParenExpression};
use crate::dts::ast::node::Name;
use crate::dts::ast::property::BitsSpec;
use crate::dts::ast::{ast_node, impl_from_str};
use crate::dts::syntax::SyntaxKind::*;
use crate::dts::syntax::{Lang, Parser, SyntaxKind, SyntaxToken};
use rowan::ast::AstNode;
use rowan::{Language, TextRange};

#[derive(Debug)]
pub enum Reference {
    Ref(Ref),
    RefPath(RefPath),
}

impl AstNode for Reference {
    type Language = Lang;

    fn can_cast(kind: SyntaxKind) -> bool
    where
        Self: Sized,
    {
        matches!(kind, REF | REF_PATH)
    }

    fn cast(node: rowan::SyntaxNode<Self::Language>) -> Option<Self>
    where
        Self: Sized,
    {
        match node.kind() {
            REF => Some(Reference::Ref(Ref::cast_unchecked(node))),
            REF_PATH => Some(Reference::RefPath(RefPath::cast_unchecked(node))),
            _ => None,
        }
    }

    fn syntax(&self) -> &rowan::SyntaxNode<Self::Language> {
        match self {
            Reference::Ref(reference) => reference.syntax(),
            Reference::RefPath(ref_path) => ref_path.syntax(),
        }
    }
}

impl Reference {
    pub fn text(&self) -> String {
        match self {
            Reference::Ref(reference) => reference.0.text().to_string(),
            Reference::RefPath(reference) => reference.0.text().to_string(),
        }
    }

    pub fn range(&self) -> TextRange {
        match self {
            Reference::Ref(reference) => reference.range(),
            Reference::RefPath(reference) => reference.range(),
        }
    }
}

ast_node! {
    struct Ref(REF);
}

impl Ref {
    pub fn target(&self) -> String {
        self.0.last_token().unwrap().to_string()
    }
}

ast_node! {
    struct Path(PATH);
}

impl Path {
    pub fn items(&self) -> impl Iterator<Item = Name> {
        self.0.children().filter_map(Name::cast)
    }
}

ast_node! {
    struct RefPath(REF_PATH);
}

impl RefPath {
    pub fn target(&self) -> Path {
        self.0.first_child().and_then(Path::cast).unwrap()
    }
}

#[derive(Debug)]
pub enum CellContent {
    Number(IntConstant),
    Expression(ParenExpression),
    Reference(Reference),
}

impl AstNode for CellContent {
    type Language = Lang;

    fn can_cast(kind: <Self::Language as Language>::Kind) -> bool
    where
        Self: Sized,
    {
        matches!(kind, INT | PAREN_EXPRESSION | REF | REF_PATH)
    }

    fn cast(node: rowan::SyntaxNode<Self::Language>) -> Option<Self>
    where
        Self: Sized,
    {
        Some(match node.kind() {
            INT => CellContent::Number(IntConstant::cast_unchecked(node)),
            PAREN_EXPRESSION => CellContent::Expression(ParenExpression::cast_unchecked(node)),
            REF => CellContent::Reference(Reference::Ref(Ref::cast_unchecked(node))),
            REF_PATH => CellContent::Reference(Reference::RefPath(RefPath::cast_unchecked(node))),
            _ => return None,
        })
    }

    fn syntax(&self) -> &rowan::SyntaxNode<Self::Language> {
        match self {
            CellContent::Number(number) => number.syntax(),
            CellContent::Expression(expression) => expression.syntax(),
            CellContent::Reference(reference) => reference.syntax(),
        }
    }
}

ast_node! {
    struct Cell(CELL);
}

impl_from_str!(Cell => Parser::parse_cell);

impl Cell {
    pub fn bits(&self) -> Option<BitsSpec> {
        self.0.children().filter_map(BitsSpec::cast).next()
    }

    pub fn inner(&self) -> CellInner {
        self.0
            .children()
            .filter_map(CellInner::cast)
            .next()
            .unwrap()
    }

    pub fn l_chev(&self) -> Option<SyntaxToken> {
        self.inner().l_chev()
    }

    pub fn r_chev(&self) -> Option<SyntaxToken> {
        self.inner().r_chev()
    }

    pub fn content(&self) -> impl Iterator<Item = CellContent> {
        self.inner().content()
    }
}

ast_node! {
    struct CellInner(CELL_INNER);
}

impl CellInner {
    pub fn l_chev(&self) -> Option<SyntaxToken> {
        self.0.first_token()
    }

    pub fn r_chev(&self) -> Option<SyntaxToken> {
        self.0.last_token()
    }

    pub fn content(&self) -> impl Iterator<Item = CellContent> {
        self.0.children().filter_map(CellContent::cast)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::cell::{Cell, CellContent, Reference};
    use crate::dts::eval::Eval;
    use crate::dts::lex::lex;
    use crate::dts::syntax::Parser;
    use assert_matches::assert_matches;
    use itertools::Itertools;
    use rowan::ast::AstNode;
    use rowan::{TextRange, TextSize};

    fn parse_to_cell(value: &str) -> Cell {
        let (ast, errors) = Parser::new(lex(value).into_iter()).parse(Parser::parse_cell);
        assert!(errors.is_empty(), "Got errors {:?}", errors);
        Cell::cast(ast).unwrap()
    }

    #[test]
    fn check_empty_cell() {
        let cell = parse_to_cell("<>");
        assert_eq!(cell.content().count(), 0);
        assert_eq!(
            cell.l_chev().unwrap().text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(1))
        );
        assert_eq!(
            cell.r_chev().unwrap().text_range(),
            TextRange::new(TextSize::new(1), TextSize::new(2))
        );
    }

    #[test]
    fn check_cell_with_single_element() {
        let cell = parse_to_cell("<&some_name>");
        let content = cell.content().collect_vec();
        assert_eq!(content.len(), 1);
        match &content[0] {
            CellContent::Reference(Reference::Ref(reference)) => {
                assert_eq!(reference.target(), "some_name".to_owned())
            }
            _ => panic!("Expected reference"),
        }
        assert_eq!(
            cell.l_chev().unwrap().text_range(),
            TextRange::new(TextSize::new(0), TextSize::new(1))
        );
        assert_eq!(
            cell.r_chev().unwrap().text_range(),
            TextRange::new(TextSize::new(11), TextSize::new(12))
        );
    }

    #[test]
    fn check_cell_with_homogeneous_elements() {
        let cell = parse_to_cell("<8 9>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 2);
        assert!(contents
            .iter()
            .all(|content| matches!(content, CellContent::Number(_))));

        let cell = parse_to_cell("<&node_a &node_b>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 2);
        assert!(contents
            .iter()
            .all(|content| matches!(content, CellContent::Reference(_))));
    }

    #[test]
    fn check_cell_with_heterogeneous_elements() {
        let cell = parse_to_cell("<17 &label>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 2);
        match &contents[0] {
            CellContent::Number(number) => {
                assert_eq!(number.eval(), Ok(17_u64))
            }
            _ => panic!("Expected number"),
        }
        assert_matches!(contents[1], CellContent::Reference(_));
    }

    #[test]
    fn check_cell_with_expression() {
        let cell = parse_to_cell("<(42 + 69)>");
        let contents = cell.content().collect_vec();
        assert_eq!(contents.len(), 1);
        match &contents[0] {
            CellContent::Expression(expr) => {
                assert_eq!(expr.eval(), Ok(111))
            }
            _ => panic!("Expected expression"),
        }
    }

    #[test]
    fn check_cell_with_bits() {
        let cell = parse_to_cell("/bits/ 8 <32>");
        let contents = cell.content().collect_vec();
        assert!(cell.bits().is_some());
        assert_eq!(contents.len(), 1);
        assert_matches!(contents[0], CellContent::Number(_));
    }
}
