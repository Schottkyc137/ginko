pub mod expression;
pub mod property;

use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::expression::IntEvalError;
use crate::dts::ErrorCode;
use line_index::TextRange;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};

#[derive(Debug, Eq, PartialEq)]
pub struct EvalError<E> {
    pub cause: E,
    pub pos: TextRange,
}

impl<E> From<EvalError<E>> for Diagnostic
where
    E: Display,
{
    fn from(value: EvalError<E>) -> Self {
        Diagnostic::new(value.pos, ErrorCode::IntError, value.cause.to_string())
    }
}

impl<E> Display for EvalError<E>
where
    E: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at pos {:?}", self.cause, self.pos)
    }
}

pub type Result<T, E> = std::result::Result<T, EvalError<E>>;

pub trait Eval<T, E> {
    fn eval(&self) -> Result<T, E>;
}

pub trait InfallibleEval<T> {
    fn value(&self) -> T;
}

impl<I, T> InfallibleEval<T> for I
where
    I: Eval<T, Infallible>,
{
    fn value(&self) -> T {
        self.eval().unwrap()
    }
}
