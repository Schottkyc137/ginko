mod expression;
mod property;

use line_index::TextRange;

#[derive(Debug, Eq, PartialEq)]
pub struct EvalError<E> {
    pub cause: E,
    pub pos: TextRange,
}

pub type Result<T, E> = std::result::Result<T, EvalError<E>>;

pub trait Eval<T, E> {
    fn eval(&self) -> Result<T, E>;
}
