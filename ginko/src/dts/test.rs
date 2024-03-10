use crate::dts::analysis::AnalysisContext;
use crate::dts::lexer::{Lexer, Token};
use crate::dts::reader::{ByteReader, Reader};
use crate::dts::{Analysis, Diagnostic, FileType, HasSpan, Parser, Position, Span};
use std::sync::Arc;

#[derive(Clone)]
pub struct Code {
    pos: Span,
    code: String,
    source: Arc<str>,
}

impl HasSpan for Code {
    fn span(&self) -> Span {
        self.pos
    }
}

fn substr_range(source: &str, range: Span, substr: &str, occurrence: usize) -> Span {
    let mut reader = ByteReader::from_string(source.into());
    let mut count = occurrence;

    reader.seek(range.start());

    while reader.pos() < range.end() {
        if reader.matches(substr) {
            count -= 1;
            if count == 0 {
                let start = reader.pos();
                for _ in substr.chars() {
                    reader.skip();
                }
                if reader.pos() <= range.end() {
                    return Span::new(start, reader.pos());
                }
            }
        }

        reader.skip();
    }

    panic!("Could not find occurrence {occurrence} of substring {substr:?}");
}

impl Code {
    pub fn new(code: &str) -> Code {
        Code::with_file_name(code, "inline source")
    }

    pub fn with_file_name(code: &str, file_name: &str) -> Code {
        let last_pos = code
            .lines()
            .enumerate()
            .last()
            .map(|(line_no, line)| Position::new(line_no as u32, line.len() as u32))
            .unwrap_or(Position::zero());
        Code {
            pos: Span::new(Position::zero(), last_pos),
            code: code.into(),
            source: Arc::from(file_name),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn source(&self) -> Arc<str> {
        self.source.clone()
    }

    pub fn parse<F, T>(&self, parse_fn: F) -> (Result<T, Diagnostic>, Vec<Diagnostic>)
    where
        F: FnOnce(&mut Parser<ByteReader>) -> Result<T, Diagnostic>,
    {
        let mut reader = ByteReader::from_string(self.code.clone());
        reader.seek(self.pos.start());
        let lexer = Lexer::new(reader, self.source.clone());
        let mut parser = Parser::new(lexer);
        (parse_fn(&mut parser), parser.diagnostics)
    }

    pub fn parse_ok<F, T>(&self, parse_fn: F) -> (T, Vec<Diagnostic>)
    where
        F: FnOnce(&mut Parser<ByteReader>) -> Result<T, Diagnostic>,
    {
        let (res, diagnostics) = self.parse(parse_fn);
        (res.expect("Unexpectedly found non-ok value"), diagnostics)
    }

    pub fn parse_ok_no_diagnostics<F, T>(&self, parse_fn: F) -> T
    where
        F: FnOnce(&mut Parser<ByteReader>) -> Result<T, Diagnostic>,
    {
        let (res, diagnostics) = self.parse(parse_fn);
        assert!(
            diagnostics.is_empty(),
            "Found unexpected diagnostics {diagnostics:?}"
        );
        res.expect("Unexpectedly found non-ok value")
    }

    pub fn get_analyzed_file(&self) -> (Vec<Diagnostic>, AnalysisContext) {
        let (file, mut diagnostics) = self.parse_ok(Parser::file);
        let mut analysis = Analysis::new(FileType::DtSource);
        analysis.analyze_file(&mut diagnostics, &file);
        let context = analysis.into_context();
        (diagnostics, context)
    }

    fn in_range(&self, span: Span) -> Code {
        Code {
            code: self.code.clone(),
            pos: span,
            source: self.source.clone(),
        }
    }

    pub fn s(&self, substr: &str, occurrence: usize) -> Code {
        let rng = substr_range(&self.code, self.pos, substr, occurrence);
        self.in_range(rng)
    }

    pub fn s1(&self, substr: &str) -> Code {
        self.s(substr, 1)
    }

    pub fn token(&self) -> Token {
        let mut byte_reader = ByteReader::from_string(self.code.clone());
        byte_reader.seek(self.pos.start());
        Lexer::new(byte_reader, self.source.clone())
            .next()
            .expect("Expected token")
    }
}
