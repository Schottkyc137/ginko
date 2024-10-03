use crate::dts::expression::SyntaxKind::*;
use crate::dts::expression::{SyntaxNode, SyntaxToken};
use rowan::TextRange;

macro_rules! ast_node {
    ($ast:ident, $kind:pat) => {
        #[derive(PartialEq, Eq, Hash)]
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
    };
}

ast_node!(IntConstant, INT);

impl IntConstant {
    pub fn text(&self) -> String {
        match self.0.green().children().next() {
            Some(rowan::NodeOrToken::Token(token)) => token.text().to_string(),
            _ => unreachable!(),
        }
    }
}

ast_node!(Constant, INT);

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

ast_node!(Primary, INT | PAREN_EXPRESSION);

pub enum PrimaryKind {
    Constant(Constant),
    Expression(ParenExpression),
}

impl Primary {
    pub fn kind(&self) -> PrimaryKind {
        match self.0.kind() {
            INT => PrimaryKind::Constant(Constant::cast_unchecked(self.0.clone())),
            PAREN_EXPRESSION => {
                PrimaryKind::Expression(ParenExpression::cast_unchecked(self.0.clone()))
            }
            _ => unreachable!(),
        }
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

ast_node!(BinaryExpression, BINARY);

impl BinaryExpression {
    pub fn lhs(&self) -> Expression {
        Expression::cast_unchecked(self.0.children().nth(0).unwrap())
    }

    pub fn op(&self) -> BinaryOp {
        let op = self.0.children().nth(1).unwrap();
        match op.first_token().unwrap().kind() {
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
            _ => unreachable!(),
        }
    }

    pub fn rhs(&self) -> Expression {
        Expression::cast_unchecked(self.0.children().nth(2).unwrap())
    }
}

ast_node!(ParenExpression, PAREN_EXPRESSION);

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

ast_node!(Expression, BINARY | INT | PAREN_EXPRESSION);

pub enum ExpressionKind {
    Binary(BinaryExpression),
    Primary(Primary),
}

impl Expression {
    pub fn kind(&self) -> ExpressionKind {
        match self.0.kind() {
            INT | PAREN_EXPRESSION => {
                ExpressionKind::Primary(Primary::cast_unchecked(self.0.clone()))
            }
            BINARY => ExpressionKind::Binary(BinaryExpression::cast_unchecked(self.0.clone())),
            _ => unreachable!(),
        }
    }
}

ast_node!(Root, ROOT);

impl Root {
    pub fn expr(&self) -> Expression {
        Expression::cast_unchecked(self.0.first_child().unwrap())
    }
}
