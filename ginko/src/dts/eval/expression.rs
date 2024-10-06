use crate::dts::ast::expression::{
    BinaryExpression, BinaryOp, Constant, ConstantKind, Expression, ExpressionKind, IntConstant,
    ParenExpression, Primary, PrimaryKind, UnaryExpression, UnaryOp,
};
use crate::dts::eval::{Eval, EvalError};
use line_index::TextRange;
use std::fmt::{Display, Formatter};
use std::num::ParseIntError;

#[derive(Debug, Eq, PartialEq)]
pub enum IntEvalError {
    ParseError(ParseIntError),
    DivideByZero,
}

impl Display for IntEvalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IntEvalError::ParseError(err) => write!(f, "{err}"),
            IntEvalError::DivideByZero => write!(f, "Divide by zero"),
        }
    }
}

pub type Result<T = u64> = crate::dts::eval::Result<T, IntEvalError>;

pub trait IntoEvalResult<T, E> {
    fn into_eval_result(self, pos: TextRange) -> crate::dts::eval::Result<T, E>;
}

impl<T> IntoEvalResult<T, IntEvalError> for std::result::Result<T, ParseIntError> {
    fn into_eval_result(self, pos: TextRange) -> Result<T> {
        match self {
            Ok(res) => Ok(res),
            Err(err) => Err(EvalError {
                cause: IntEvalError::ParseError(err),
                pos,
            }),
        }
    }
}

macro_rules! int_eval {
    ($($t:ident),+) => {
        $(
            impl Eval<$t, IntEvalError> for IntConstant {
                fn eval(&self) -> Result<$t> {
                    // TODO: suffixes (i.e., L, LL, ULL, ...)
                    let text = self.text();
                    // guard against '0' case being matched in octal
                    if text == "0" {
                        Ok(0)
                    } else if let Some(digits) = text.to_ascii_lowercase().strip_prefix("0x") {
                        $t::from_str_radix(digits, 16).into_eval_result(self.range())
                    } else if let Some(digits) = text.strip_prefix("0") {
                        $t::from_str_radix(digits, 8).into_eval_result(self.range())
                    } else {
                        text.parse::<$t>().into_eval_result(self.range())
                    }
                }
            }
        )+
    };
}

int_eval!(u8, u16, u32, u64);

impl Eval<u64, IntEvalError> for Constant {
    fn eval(&self) -> Result {
        match self.kind() {
            ConstantKind::Int(int) => int.eval(),
        }
    }
}

impl Eval<u64, IntEvalError> for Primary {
    fn eval(&self) -> Result {
        match self.kind() {
            PrimaryKind::Constant(c) => c.eval(),
            PrimaryKind::ParenExpression(expr) => expr.eval(),
        }
    }
}

impl Eval<u64, IntEvalError> for BinaryExpression {
    fn eval(&self) -> Result {
        let lhs = self.lhs().eval()?;
        let rhs = self.rhs().eval()?;
        Ok(match self.bin_op() {
            BinaryOp::Plus => lhs.wrapping_add(rhs),
            BinaryOp::Minus => lhs.wrapping_sub(rhs),
            BinaryOp::Mult => lhs.wrapping_mul(rhs),
            BinaryOp::Div => {
                if rhs == 0 {
                    return Err(EvalError {
                        pos: self.rhs().range(),
                        cause: IntEvalError::DivideByZero,
                    });
                }
                lhs.wrapping_div(rhs)
            }
            BinaryOp::Mod => {
                if rhs == 0 {
                    return Err(EvalError {
                        pos: self.rhs().range(),
                        cause: IntEvalError::DivideByZero,
                    });
                }
                lhs.wrapping_rem(rhs)
            }
            BinaryOp::LShift => lhs << rhs,
            BinaryOp::RShift => lhs >> rhs,
            BinaryOp::Gt => {
                if lhs > rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Gte => {
                if lhs >= rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Lt => {
                if lhs < rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Lte => {
                if lhs <= rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Eq => {
                if lhs == rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Neq => {
                if lhs != rhs {
                    1
                } else {
                    0
                }
            }
            BinaryOp::And => lhs & rhs,
            BinaryOp::Or => lhs | rhs,
            BinaryOp::Xor => lhs ^ rhs,
            BinaryOp::LAnd => {
                if (lhs != 0) && (rhs != 0) {
                    1
                } else {
                    0
                }
            }
            BinaryOp::Lor => {
                if (lhs != 0) || (rhs != 0) {
                    1
                } else {
                    0
                }
            }
        })
    }
}

impl Eval<u64, IntEvalError> for UnaryExpression {
    fn eval(&self) -> Result {
        let result = self.expr().eval()?;
        Ok(match self.unary_op() {
            UnaryOp::Minus => 0_u64.wrapping_sub(result),
            UnaryOp::LNot => {
                if result == 0 {
                    1
                } else {
                    0
                }
            }
            UnaryOp::BitNot => !result,
        })
    }
}

impl Eval<u64, IntEvalError> for ParenExpression {
    fn eval(&self) -> Result {
        self.expr().eval()
    }
}

impl Eval<u64, IntEvalError> for Expression {
    fn eval(&self) -> Result {
        match self.kind() {
            ExpressionKind::Binary(binary) => binary.eval(),
            ExpressionKind::Unary(unary) => unary.eval(),
            ExpressionKind::Primary(primary) => primary.eval(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::expression::Expression;
    use crate::dts::ast::Cast;
    use crate::dts::eval::Eval;
    use crate::dts::lex::lex::lex;
    use crate::dts::syntax::Parser;

    fn check_equal(expression: &str, result: u64) {
        let (ast, diag) = Parser::new(lex(expression).into_iter()).parse(Parser::parse_expression);
        assert!(diag.is_empty());
        let expr = Expression::cast(ast).unwrap();
        assert_eq!(expr.eval(), Ok(result))
    }

    #[test]
    fn eval_simple_expressions() {
        check_equal("1", 1);
        check_equal("0xA", 10);
        check_equal("077", 63);
        check_equal("0xdeadbeef", 0xdeadbeef);
    }

    #[test]
    fn eval_binary_expression() {
        check_equal("1 + 1", 2);
        check_equal("7 * 3", 21);
        check_equal("1 || 0", 1);
        check_equal("4 / 2", 2);
        check_equal("10 / 3", 3);
        check_equal("19 % 4", 3);
        check_equal("1 << 13", 0x2000);
        check_equal("0x1000 >> 4", 0x100);

        check_equal("1 < 2", 1);
        check_equal("2 < 1", 0);
        check_equal("1 < 1", 0);

        check_equal("1 <= 2", 1);
        check_equal("2 <= 1", 0);
        check_equal("1 <= 1", 1);

        check_equal("1 > 2", 0);
        check_equal("2 > 1", 1);
        check_equal("1 > 1", 0);

        check_equal("1 >= 2", 0);
        check_equal("2 >= 1", 1);
        check_equal("1 >= 1", 1);

        check_equal("1 == 1", 1);
        check_equal("1 == 2", 0);

        check_equal("1 != 1", 0);
        check_equal("1 != 2", 1);

        check_equal("0xdeadbeef & 0xffff0000", 0xdead0000);
        check_equal("0xA7B8C9DA ^ 0xf0f0f0f0", 0x5748392A);
        check_equal("0xabcd0000 | 0x0000abcd", 0xabcdabcd);

        check_equal("0 && 42", 0);
        check_equal("42 && 0", 0);
        check_equal("42 && 42", 1);
        check_equal("0 && 0", 0);

        check_equal("0 || 42", 1);
        check_equal("42 || 0", 1);
        check_equal("42 || 42", 1);
        check_equal("0 || 0", 0);
    }

    #[test]
    fn eval_nested_binary_expression() {
        check_equal("2 + 3 * 4", 14);
        check_equal("(2 + 3) * 4", 20);
        check_equal("3 * 4 + 2", 14);
        check_equal("3 * (4 + 2)", 18);

        check_equal("123456790 - 4/2 + 17%4", 123456789);
    }

    #[test]
    fn eval_unary_expression() {
        check_equal("~0xAB", 0xFFFFFFFFFFFFFF54);
        check_equal("!0", 1);
        check_equal("!1", 0);
        check_equal("!!42", 1);
        check_equal("!!!42", 0);
        check_equal("~0xFFFFFFFFFFFFFFFF", 0);
    }

    #[test]
    fn unary_expressions_associativity() {
        check_equal("~!0xFFFFFFF", 0xFFFFFFFFFFFFFFFF);
        check_equal("!~0xFFFFFFF", 0);
    }

    #[test]
    fn unary_ops_in_binary_ops() {
        check_equal("4 + -3", 1);
    }
}
