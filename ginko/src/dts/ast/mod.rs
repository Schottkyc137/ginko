pub mod cell;
pub mod expression;
pub mod file;
pub mod label;
pub mod node;
pub mod property;

pub use cell::{Cell, CellContent, CellInner, Path, Ref, RefPath, Reference};
pub use expression::{
    BinaryExpression, BinaryOp, Constant, Expression, ExpressionKind, IntConstant, Op,
    ParenExpression, Primary, UnaryExpression, UnaryOp,
};
pub use file::{File, FileItemKind, Header, HeaderKind, Include, ReserveMemory};
pub use label::Label;
pub use node::{Decoration, DeleteSpec, Name, NameOrRef, Node, NodeBody, NodeOrProperty, Property};
pub use property::{
    BitsSpec, ByteChunk, ByteString, PropertyList, PropertyValue, PropertyValueKind, StringProperty,
};
pub use rowan::ast::AstNode;

macro_rules! ast_node {
    (struct $ast:ident($kind:pat);) => {
        #[derive(PartialEq, Eq, Hash, Debug, Clone)]
        #[repr(transparent)]
        pub struct $ast($crate::dts::syntax::SyntaxNode);
        impl $ast {
            #[allow(unused)]
            pub fn range(&self) -> rowan::TextRange {
                self.0.text_range()
            }

            #[allow(unused)]
            pub(crate) fn cast_unchecked(node: $crate::dts::syntax::SyntaxNode) -> Self {
                debug_assert!(matches!(node.kind(), $kind), "got {}", node.kind());
                Self(node)
            }
        }

        impl rowan::ast::AstNode for $ast {
            type Language = $crate::dts::syntax::Lang;

            fn cast(node: $crate::dts::syntax::SyntaxNode) -> Option<Self> {
                match node.kind() {
                    $kind => Some(Self(node)),
                    _ => None,
                }
            }

            fn can_cast(kind: $crate::dts::syntax::SyntaxKind) -> bool {
                matches!(kind, $kind)
            }

            fn syntax(&self) -> &$crate::dts::syntax::SyntaxNode {
                &self.0
            }
        }

        impl std::fmt::Display for $ast {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };

    (terminal struct $ast:ident($kind:pat);) => {
        ast_node!(
            struct $ast($kind);
        );
        impl $ast {
            pub fn text(&self) -> String {
                self.0.first_token().unwrap().text().to_string()
            }
        }
    };
}

macro_rules! impl_from_str {
    ($name:ident => $fn_name:expr) => {
        impl std::str::FromStr for $name {
            type Err = Vec<$crate::dts::diagnostics::Diagnostic>;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let (ast, errors) =
                    $crate::dts::syntax::Parser::new($crate::dts::lex::lex(s).into_iter())
                        .parse($fn_name);
                if errors.is_empty() {
                    // TODO: unwrap or diagnostic?
                    Ok($name::cast(ast).expect("Found non-expecting root object"))
                } else {
                    Err(errors)
                }
            }
        }
    };
}

pub(crate) use ast_node;
pub(crate) use impl_from_str;
