use crate::dts::expression::SyntaxKind::*;
use crate::dts::expression::{SyntaxNode, SyntaxToken};
use rowan::TextRange;
use std::fmt::Formatter;

macro_rules! ast_node {
    (struct $ast:ident($kind:pat);) => {
        #[derive(PartialEq, Eq, Hash, Debug)]
        #[repr(transparent)]
        pub struct $ast(SyntaxNode);
        impl $ast {
            #[allow(unused)]
            pub fn cast(node: SyntaxNode) -> Option<Self> {
                match node.kind() {
                    $kind => Some(Self(node)),
                    _ => None,
                }
            }

            #[allow(unused)]
            fn cast_unchecked(node: SyntaxNode) -> Self {
                debug_assert!(matches!(node.kind(), $kind), "got {}", node.kind());
                Self(node)
            }

            #[allow(unused)]
            pub fn range(&self) -> TextRange {
                self.0.text_range()
            }
        }

        impl std::fmt::Display for $ast {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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

ast_node! {
    terminal struct IntConstant(INT);
}

ast_node! {
    struct Constant(INT);
}

pub enum ConstantKind {
    Int(IntConstant),
}

impl Constant {
    pub fn kind(&self) -> ConstantKind {
        match self.0.kind() {
            INT => ConstantKind::Int(IntConstant::cast_unchecked(self.0.clone())),
            _ => unreachable!(),
        }
    }
}

ast_node! {
    struct Primary(INT | PAREN_EXPRESSION);
}

pub enum PrimaryKind {
    Constant(Constant),
    ParenExpression(ParenExpression),
}

impl Primary {
    pub fn kind(&self) -> PrimaryKind {
        match self.0.kind() {
            INT => PrimaryKind::Constant(Constant::cast_unchecked(self.0.clone())),
            PAREN_EXPRESSION => {
                PrimaryKind::ParenExpression(ParenExpression::cast_unchecked(self.0.clone()))
            }
            _ => unreachable!(),
        }
    }
}

ast_node! {
    terminal struct Op(OP);
}

impl Op {
    pub fn binary_op(&self) -> Option<BinaryOp> {
        Some(match self.0.first_token().unwrap().kind() {
            PLUS => BinaryOp::Plus,
            MINUS => BinaryOp::Minus,
            STAR => BinaryOp::Mult,
            SLASH => BinaryOp::Div,
            PERCENT => BinaryOp::Mod,
            DOUBLE_LT => BinaryOp::LShift,
            DOUBLE_GT => BinaryOp::RShift,
            GT => BinaryOp::Gt,
            GTE => BinaryOp::Gte,
            LT => BinaryOp::Lt,
            LTE => BinaryOp::Lte,
            EQ => BinaryOp::Eq,
            NEQ => BinaryOp::Neq,
            BAR => BinaryOp::Or,
            AMP => BinaryOp::And,
            CIRC => BinaryOp::Xor,
            DOUBLE_BAR => BinaryOp::Lor,
            DOUBLE_AMP => BinaryOp::LAnd,
            _ => return None,
        })
    }

    pub fn unary_op(&self) -> Option<UnaryOp> {
        Some(match self.0.first_token().unwrap().kind() {
            MINUS => UnaryOp::Minus,
            EXCLAMATION => UnaryOp::LNot,
            TILDE => UnaryOp::BitNot,
            _ => return None,
        })
    }
}

pub enum BinaryOp {
    Plus,
    Minus,
    Mult,
    Div,
    Mod,
    LShift,
    RShift,
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
    Neq,
    And,
    Or,
    Xor,
    LAnd,
    Lor,
}

ast_node! {
    struct BinaryExpression(BINARY);
}

impl BinaryExpression {
    pub fn lhs(&self) -> Expression {
        Expression::cast_unchecked(self.0.children().nth(0).unwrap())
    }

    pub fn op(&self) -> Op {
        Op::cast_unchecked(self.0.children().nth(1).unwrap())
    }

    pub fn bin_op(&self) -> BinaryOp {
        self.op().binary_op().unwrap()
    }

    pub fn rhs(&self) -> Expression {
        Expression::cast_unchecked(self.0.children().nth(2).unwrap())
    }
}

pub enum UnaryOp {
    Minus,
    LNot,
    BitNot,
}

ast_node! {
    struct UnaryExpression(UNARY);
}

impl UnaryExpression {
    pub fn expr(&self) -> Expression {
        Expression::cast_unchecked(self.0.children().nth(1).unwrap())
    }

    pub fn op(&self) -> Op {
        Op::cast_unchecked(self.0.children().nth(0).unwrap())
    }

    pub fn unary_op(&self) -> UnaryOp {
        self.op().unary_op().unwrap()
    }
}

ast_node! {
    struct ParenExpression(PAREN_EXPRESSION);
}

impl ParenExpression {
    pub fn l_par(&self) -> SyntaxToken {
        self.0.first_token().unwrap()
    }

    pub fn expr(&self) -> Expression {
        Expression::cast_unchecked(self.0.first_child().unwrap())
    }

    pub fn r_par(&self) -> SyntaxToken {
        self.0.last_token().unwrap()
    }
}

ast_node! {
    struct Expression(UNARY | BINARY | INT | PAREN_EXPRESSION);
}

pub enum ExpressionKind {
    Binary(BinaryExpression),
    Unary(UnaryExpression),
    Primary(Primary),
}

impl Expression {
    pub fn kind(&self) -> ExpressionKind {
        match self.0.kind() {
            INT | PAREN_EXPRESSION => {
                ExpressionKind::Primary(Primary::cast_unchecked(self.0.clone()))
            }
            UNARY => ExpressionKind::Unary(UnaryExpression::cast_unchecked(self.0.clone())),
            BINARY => ExpressionKind::Binary(BinaryExpression::cast_unchecked(self.0.clone())),
            _ => unreachable!(),
        }
    }
}
