use crate::dts::analysis::{Analysis, AnalysisContext, AnalysisResult};
use crate::dts::data::HasSource;
use crate::dts::reader::{ByteReader, Reader};
use crate::dts::tokens::{Lexer, Token};
use crate::dts::{Diagnostic, FileType, HasSpan, Parser, ParserContext, Position, Project, Span};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct Code {
    pos: Span,
    code: String,
    source: Arc<Path>,
    parser_context: ParserContext,
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

impl HasSource for Code {
    fn source(&self) -> Arc<Path> {
        self.source.clone()
    }
}

impl Code {
    pub fn new(code: &str) -> Code {
        Code::with_file_name(code, "inline source", ParserContext::default())
    }

    #[allow(unused)]
    pub fn with_context(code: &str, context: ParserContext) -> Code {
        Code::with_file_name(code, "inline source", context)
    }

    pub fn with_file_name(code: &str, file_name: &str, context: ParserContext) -> Code {
        let last_pos = code
            .lines()
            .enumerate()
            .last()
            .map(|(line_no, line)| Position::new(line_no as u32, line.len() as u32))
            .unwrap_or(Position::zero());
        Code {
            pos: Span::new(Position::zero(), last_pos),
            code: code.into(),
            source: Arc::from(PathBuf::from(file_name)),
            parser_context: context,
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn parse<F, T>(&self, parse_fn: F) -> (Result<T, Diagnostic>, Vec<Diagnostic>)
    where
        F: FnOnce(&mut Parser<ByteReader>) -> Result<T, Diagnostic>,
    {
        let mut reader = ByteReader::from_string(self.code.clone());
        reader.seek(self.pos.start());
        let lexer = Lexer::new(reader, self.source.clone());
        let mut parser = Parser::new(lexer, self.parser_context.clone());
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
        let (file, mut parse_diagnostics) = self.parse_ok(Parser::file);
        let fake_project = Project::default();
        let mut analysis = Analysis::new();
        let AnalysisResult {
            context,
            mut diagnostics,
            ..
        } = analysis.analyze_file(&file, FileType::DtSource, &fake_project);
        diagnostics.append(&mut parse_diagnostics);
        (diagnostics, context)
    }

    fn in_range(&self, span: Span) -> Code {
        Code {
            code: self.code.clone(),
            pos: span,
            source: self.source.clone(),
            parser_context: self.parser_context.clone(),
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
