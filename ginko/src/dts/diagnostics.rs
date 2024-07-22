use crate::dts::data::{HasSource, HasSpan, Span};
use crate::dts::error_codes::{ErrorCode, SeverityMap};
use crate::dts::import_guard::CyclicDependencyError;
use crate::dts::tokens::{Token, TokenKind};
use itertools::Itertools;
use std::fmt::{Display, Formatter};
use std::io;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(PartialEq, Debug, Clone)]
pub enum NameContext {
    Label,
    NodeName,
    PropertyName,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

impl Display for Severity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use Severity::*;
        match self {
            Error => write!(f, "error"),
            Warning => write!(f, "warning"),
            Hint => write!(f, "hint"),
        }
    }
}

impl Display for NameContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NameContext::Label => write!(f, "label"),
            NameContext::NodeName => write!(f, "node name"),
            NameContext::PropertyName => write!(f, "property"),
        }
    }
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Semicolon => write!(f, "';'"),
            TokenKind::Slash => write!(f, "'/'"),
            TokenKind::Equal => write!(f, "'='"),
            TokenKind::OpenBracket => write!(f, "'['"),
            TokenKind::CloseBracket => write!(f, "']'"),
            TokenKind::OpenParen => write!(f, "'('"),
            TokenKind::CloseParen => write!(f, "')'"),
            TokenKind::ChevronLeft => write!(f, "'<'"),
            TokenKind::ChevronRight => write!(f, "'>'"),
            TokenKind::Comma => write!(f, "','"),
            TokenKind::OpenBrace => write!(f, "'{{'"),
            TokenKind::CloseBrace => write!(f, "'}}'"),
            TokenKind::Ident(_) => write!(f, "identifier"),
            TokenKind::Label(_) => write!(f, "label"),
            TokenKind::String(_) => write!(f, "string"),
            TokenKind::UnparsedNumber(_) => write!(f, "number"),
            TokenKind::Directive(directive) => write!(f, "{directive}"),
            TokenKind::Ref(_) => write!(f, "reference"),
            TokenKind::Comment(_) => write!(f, "comment"),
            TokenKind::Unknown(_) => write!(f, "unknown"),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct Diagnostic {
    pub kind: ErrorCode,
    pub span: Span,
    pub source: Arc<Path>,
    pub message: String,
}

impl Diagnostic {
    pub fn new(
        span: Span,
        source: Arc<Path>,
        kind: ErrorCode,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            kind,
            source,
            span,
            message: message.into(),
        }
    }

    pub fn io_error(span: Span, source: Arc<Path>, err: io::Error) -> Diagnostic {
        Diagnostic::new(span, source, ErrorCode::IOError, format!("{}", err))
    }

    pub fn from_token(token: Token, kind: ErrorCode, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            kind,
            source: token.source(),
            span: token.span,
            message: message.into(),
        }
    }

    pub fn parse_int_error(span: Span, source: Arc<Path>, err: ParseIntError) -> Diagnostic {
        Diagnostic::new(span, source, ErrorCode::IntError, format!("{err}"))
    }

    pub fn cyclic_dependency_error(
        span: Span,
        source: Arc<Path>,
        err: CyclicDependencyError<PathBuf>,
    ) -> Diagnostic {
        let str = err
            .cycle()
            .iter()
            .map(|element| format!("{}", element.display()))
            .join(" -> ");
        Diagnostic::new(
            span,
            source,
            ErrorCode::CyclicDependencyError,
            format!("Cyclic include: {str}"),
        )
    }

    pub fn expected(span: Span, source: Arc<Path>, kinds: &[TokenKind]) -> Diagnostic {
        let msg = if kinds.len() == 1 {
            format!("Expected {}", kinds[0])
        } else {
            format!(
                "Expected one of {}",
                kinds.iter().map(|kind| format!("{kind}")).join(", ")
            )
        };
        Diagnostic::new(span, source, ErrorCode::Expected, msg)
    }

    pub fn kind(&self) -> &ErrorCode {
        &self.kind
    }

    pub fn severity(&self, map: &SeverityMap) -> Severity {
        map[self.kind]
    }
}

impl HasSpan for Diagnostic {
    fn span(&self) -> Span {
        self.span
    }
}

impl HasSource for Diagnostic {
    fn source(&self) -> Arc<Path> {
        self.source.clone()
    }
}

pub struct DiagnosticPrinter<'a> {
    pub diagnostics: &'a [Diagnostic],
    pub code: Vec<String>,
    pub severity_map: SeverityMap,
}

impl<'a> DiagnosticPrinter<'a> {
    fn fmt_diagnostic(&self, f: &mut Formatter<'_>, diagnostic: &Diagnostic) -> std::fmt::Result {
        let start = diagnostic.span.start();
        let end = diagnostic.span.end();
        debug_assert!(start.line() == end.line());
        let empty_string = "".to_string();
        let line = self
            .code
            .get(start.line() as usize)
            .unwrap_or(&empty_string)
            .clone();
        // take tabs into consideration
        let line_empty: String = line
            .chars()
            .map(|ch| if !ch.is_ascii_whitespace() { ' ' } else { ch })
            .take(start.character() as usize)
            .collect();

        let prefix = format!("{}", start.line() + 1);
        let prefix_empty = " ".repeat(prefix.len());
        writeln!(
            f,
            "{} --> {}:{}:{}",
            diagnostic.severity(&self.severity_map),
            diagnostic.source.to_string_lossy(),
            start.line() + 1,
            start.character() + 1
        )?;
        writeln!(f, "{} |", prefix_empty)?;
        writeln!(f, "{} | {}", prefix, line)?;
        let len = if start.character() == end.character() {
            1
        } else {
            end.character() - start.character()
        };
        write!(
            f,
            "{} | {}{}",
            prefix_empty,
            line_empty,
            "^".repeat(len as usize)
        )?;
        write!(f, " {}", diagnostic.message)?;
        Ok(())
    }
}

impl<'a> Display for DiagnosticPrinter<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for diagnostic in self.diagnostics.iter() {
            self.fmt_diagnostic(f, diagnostic)?;
            writeln!(f)?;
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::data::{HasSource, HasSpan};
    use crate::dts::diagnostics::{Diagnostic, DiagnosticPrinter, ErrorCode};
    use crate::dts::error_codes::SeverityMap;
    use crate::dts::parser::Parser;
    use crate::dts::test::Code;
    use crate::dts::ParserContext;
    use itertools::Itertools;

    #[test]
    fn display_missing_semicolon() {
        let code = Code::with_file_name("/ {}", "fname", ParserContext {
          include_paths: Vec::new(),
      },);
        let (_, diag) = code.parse(
            Parser::file
        );
        assert_eq!(
            diag,
            vec![Diagnostic::new(
                code.s1("}").end().as_span(),
                code.source(),
                ErrorCode::Expected,
                "Expected ';'"
            )]
        );
        let printer = DiagnosticPrinter {
            diagnostics: &diag,
            code: vec!["/ {}".into()],
            severity_map: SeverityMap::default(),
        };
        let formatter_err = "\
error --> fname:1:5
  |
1 | / {}
  |     ^ Expected ';'

"
        .to_string();
        assert_eq!(formatter_err, format!("{printer}"));
    }

    #[test]
    fn display_warning_message() {
        let code = Code::with_file_name(
            "\
        /dts-v1/;

        / {
            very-long-company,very-long-name;
        };",
            "fname", ParserContext {
              include_paths: Vec::new(),
          },
        );
        let (_, diag) = code.parse(
            Parser::file
        );
        let printer = DiagnosticPrinter {
            diagnostics: &diag,
            code: code
                .code()
                .lines()
                .map(|line| line.to_string())
                .collect_vec(),
            severity_map: SeverityMap::default(),
        };
        let formatter_err = "\
warning --> fname:4:13
  |
4 |             very-long-company,very-long-name;
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ property should only have 31 characters but has 32 characters

".to_string();
        assert_eq!(formatter_err, format!("{printer}"));
    }
}
