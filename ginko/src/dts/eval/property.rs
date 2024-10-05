use crate::dts::ast::property::{ByteChunk, ByteString};
use crate::dts::eval;
use crate::dts::eval::{Eval, EvalError};
use itertools::Itertools;
use std::num::ParseIntError;

#[derive(Debug)]
enum ByteEvalError {
    OddNumberOfBytes,
    ParseError(ParseIntError),
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

#[cfg(test)]
mod tests {
    use crate::dts::ast::property::ByteString;
    use crate::dts::eval::Eval;

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
}
