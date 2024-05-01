use crate::dts::data::Position;
use std::io;
use std::io::Read;

pub trait Reader {
    fn consume(&mut self) -> Option<u8>;

    fn peek(&self) -> Option<u8>;

    fn pos(&self) -> Position;

    fn skip(&mut self) {
        let _ = self.consume();
    }

    fn skip_if<F>(&mut self, cond: F) -> bool
    where
        F: FnOnce(u8) -> bool,
    {
        let Some(ch) = self.peek() else { return false };
        if cond(ch) {
            self.skip();
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    fn seek(&mut self, pos: Position) {
        loop {
            if self.pos() < pos {
                self.skip();
            } else {
                return;
            }
        }
    }
}

pub struct ByteReader {
    data: Box<[u8]>,
    char_pos: usize,
    pos: Position,
}

impl Reader for ByteReader {
    fn consume(&mut self) -> Option<u8> {
        let ch = *self.data.get(self.char_pos)?;
        self.char_pos += 1;
        match ch {
            b'\n' => {
                self.pos = Position::new(self.pos.line() + 1, 0);
            }
            _ => self.pos = Position::new(self.pos.line(), self.pos.character() + 1),
        }
        Some(ch)
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.char_pos).copied()
    }

    fn pos(&self) -> Position {
        self.pos
    }
}

impl ByteReader {
    pub fn from_string(string: String) -> ByteReader {
        ByteReader {
            data: string.as_bytes().into(),
            pos: Position::zero(),
            char_pos: 0,
        }
    }

    pub fn from_read(mut read: impl Read) -> Result<ByteReader, io::Error> {
        let mut str = String::new();
        read.read_to_string(&mut str)?;
        Ok(ByteReader::from_string(str))
    }

    #[cfg(test)]
    pub fn matches(&self, substr: &str) -> bool {
        if self.char_pos + substr.len() > self.data.len() {
            return false;
        }
        &self.data[self.char_pos..=(self.char_pos + substr.len() - 1)] == substr.as_bytes()
    }
}
