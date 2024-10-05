pub mod cell;
pub mod expression;
mod node;
pub mod property;

macro_rules! ast_node {
    (struct $ast:ident($kind:pat);) => {
        #[derive(PartialEq, Eq, Hash, Debug)]
        #[repr(transparent)]
        pub struct $ast($crate::dts::syntax::SyntaxNode);
        impl $ast {
            #[allow(unused)]
            pub fn cast(node: $crate::dts::syntax::SyntaxNode) -> Option<Self> {
                match node.kind() {
                    $kind => Some(Self(node)),
                    _ => None,
                }
            }

            #[allow(unused)]
            fn cast_unchecked(node: $crate::dts::syntax::SyntaxNode) -> Self {
                debug_assert!(matches!(node.kind(), $kind), "got {}", node.kind());
                Self(node)
            }

            #[allow(unused)]
            pub fn range(&self) -> rowan::TextRange {
                self.0.text_range()
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
            type Err = Vec<String>;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let (ast, errors) = Parser::new(lex(s).into_iter()).parse($fn_name);
                if errors.is_empty() {
                    Ok($name::cast(ast).unwrap())
                } else {
                    Err(errors)
                }
            }
        }
    };
}

pub(crate) use ast_node;
pub(crate) use impl_from_str;
