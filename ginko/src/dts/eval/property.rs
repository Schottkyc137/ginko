use crate::dts::ast::property::{ByteChunk, ByteString, StringProperty};
use crate::dts::eval;
use crate::dts::eval::{Eval, EvalError};
use itertools::Itertools;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::num::ParseIntError;

#[derive(Debug)]
pub enum ByteEvalError {
    OddNumberOfBytes,
    ParseError(ParseIntError),
}

impl Display for ByteEvalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ByteEvalError::OddNumberOfBytes => {
                write!(f, "Number of elements in byte string must be even")
            }
            ByteEvalError::ParseError(e) => write!(f, "{}", e),
        }
    }
}

impl Eval<Vec<u8>, ByteEvalError> for ByteChunk {
    fn eval(&self) -> eval::Result<Vec<u8>, ByteEvalError> {
        let raw_str = self.text();
        if raw_str.len() % 2 != 0 {
            return Err(EvalError {
                cause: ByteEvalError::OddNumberOfBytes,
                pos: self.range(),
            });
        };
        let mut bytes: Vec<u8> = Vec::with_capacity(raw_str.len() / 2);
        for (first, second) in raw_str.bytes().map(|ch| ch.to_ascii_lowercase()).tuples() {
            match u8::from_str_radix(std::str::from_utf8(&[first, second]).unwrap(), 16) {
                Ok(byte) => bytes.push(byte),
                Err(err) => {
                    return Err(EvalError {
                        cause: ByteEvalError::ParseError(err),
                        pos: self.range(), // TODO: The range should be where the faulty bytes are
                    });
                }
            }
        }
        Ok(bytes)
    }
}

impl Eval<Vec<u8>, ByteEvalError> for ByteString {
    fn eval(&self) -> eval::Result<Vec<u8>, ByteEvalError> {
        self.contents()
            .map(|chunk| chunk.eval())
            .fold_ok(Vec::new(), |mut old, new| {
                old.extend(new);
                old
            })
    }
}

impl Eval<String, Infallible> for StringProperty {
    fn eval(&self) -> eval::Result<String, Infallible> {
        // string guaranteed to have leading and trailing " character
        let text = self.text();
        let raw_string = text.strip_prefix('"').unwrap().strip_suffix('"').unwrap();
        // We can just kill all backslashes because a backslash can never come right before the last quote.
        // This would have lead to an error in the lexer.
        Ok(raw_string.chars().unescape('\\').collect())
    }
}

struct UnescapeItr<I>
where
    I: Iterator,
    I::Item: Eq,
{
    inner: I,
    escape: I::Item,
    escape_seq_seen: bool,
}

impl<I> UnescapeItr<I>
where
    I: Iterator,
    I::Item: Eq,
{
    pub fn new(iter: I, escape: I::Item) -> UnescapeItr<I> {
        UnescapeItr {
            inner: iter,
            escape,
            escape_seq_seen: false,
        }
    }
}

impl<I> Iterator for UnescapeItr<I>
where
    I: Iterator,
    I::Item: Eq,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next()?;
        if !self.escape_seq_seen && item == self.escape {
            self.escape_seq_seen = true;
            self.inner.next()
        } else {
            self.escape_seq_seen = false;
            Some(item)
        }
    }
}

trait UnescapeItrExtension<I>
where
    I: Iterator,
    I::Item: Eq,
{
    fn unescape(self, escape_seq: I::Item) -> UnescapeItr<I>;
}

impl<I> UnescapeItrExtension<I> for I
where
    I: Iterator,
    I::Item: Eq,
{
    fn unescape(self, escape_seq: I::Item) -> UnescapeItr<I> {
        UnescapeItr::new(self, escape_seq)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::ast::property::{ByteString, PropertyValue, PropertyValueKind};
    use crate::dts::eval::Eval;
    use crate::dts::eval::InfallibleEval;

    #[test]
    fn analyze_byte_string() {
        let byte_string = "[000012345678]".parse::<ByteString>().unwrap();
        assert_eq!(
            byte_string.eval().unwrap(),
            vec![0x00, 0x00, 0x12, 0x34, 0x56, 0x78]
        );
        let byte_string = "[00 00 12 34 56 78]".parse::<ByteString>().unwrap();
        assert_eq!(
            byte_string.eval().unwrap(),
            vec![0x00, 0x00, 0x12, 0x34, 0x56, 0x78]
        );
        let byte_string = "[AB CD]".parse::<ByteString>().unwrap();
        assert_eq!(byte_string.eval().unwrap(), vec![0xAB, 0xCD]);
    }

    #[test]
    fn analyze_simple_strings() {
        let string = r#""Hello, World!""#.parse::<PropertyValue>().unwrap();
        match string.kind() {
            PropertyValueKind::String(string) => {
                assert_eq!(string.value(), "Hello, World!")
            }
            _ => panic!("Unexpected found non-string"),
        }
    }

    #[test]
    fn analyze_strings_with_escape_sequences() {
        let string = r#""\\"""#.parse::<PropertyValue>().unwrap();
        match string.kind() {
            PropertyValueKind::String(string) => {
                assert_eq!(string.value(), "\\")
            }
            _ => panic!("Unexpected found non-string"),
        }
        let string = r#""\""""#.parse::<PropertyValue>().unwrap();
        match string.kind() {
            PropertyValueKind::String(string) => {
                assert_eq!(string.value(), "\"")
            }
            _ => panic!("Unexpected found non-string"),
        }
        let string = r#""Hello, \"World!\"""#.parse::<PropertyValue>().unwrap();
        match string.kind() {
            PropertyValueKind::String(string) => {
                assert_eq!(string.value(), "Hello, \"World!\"")
            }
            _ => panic!("Unexpected found non-string"),
        }
        let string = r#""Hello, \\World\!""#.parse::<PropertyValue>().unwrap();
        match string.kind() {
            PropertyValueKind::String(string) => {
                assert_eq!(string.value(), "Hello, \\World!")
            }
            _ => panic!("Unexpected found non-string"),
        }
        let string = r#""\\Hello, World!\\""#.parse::<PropertyValue>().unwrap();
        match string.kind() {
            PropertyValueKind::String(string) => {
                assert_eq!(string.value(), "\\Hello, World!\\")
            }
            _ => panic!("Unexpected found non-string"),
        }
    }
}
