use crate::dts::analysis::AnalysisContext;
use crate::dts::lexer::{Lexer, Token};
use crate::dts::reader::{ByteReader, Reader};
use crate::dts::{Analysis, Diagnostic, FileType, HasSpan, Parser, Position, Span};

#[derive(Clone)]
pub struct Code {
    pos: Span,
    source: String,
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
        let last_pos = code
            .lines()
            .enumerate()
            .last()
            .map(|(line_no, line)| Position::new(line_no as u32, line.len() as u32))
            .unwrap_or(Position::zero());
        Code {
            pos: Span::new(Position::zero(), last_pos),
            source: code.into(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn parse<F, T>(&self, parse_fn: F) -> (Result<T, Diagnostic>, Vec<Diagnostic>)
    where
        F: FnOnce(&mut Parser<ByteReader>) -> Result<T, Diagnostic>,
    {
        let mut reader = ByteReader::from_string(self.source.clone());
        reader.seek(self.pos.start());
        let lexer = Lexer::new(reader);
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
            source: self.source.clone(),
            pos: span,
        }
    }

    pub fn s(&self, substr: &str, occurrence: usize) -> Code {
        let rng = substr_range(&self.source, self.pos, substr, occurrence);
        self.in_range(rng)
    }

    pub fn s1(&self, substr: &str) -> Code {
        self.s(substr, 1)
    }

    pub fn token(&self) -> Token {
        let mut byte_reader = ByteReader::from_string(self.source.clone());
        byte_reader.seek(self.pos.start());
        Lexer::new(byte_reader).next().expect("Expected token")
    }
}
