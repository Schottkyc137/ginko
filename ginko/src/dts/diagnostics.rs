use crate::dts::ast::CompilerDirective;
use crate::dts::data::{HasSource, HasSpan, Span};
use crate::dts::importer::CyclicDependencyError;
use crate::dts::lexer::{Token, TokenKind};
use itertools::Itertools;
use std::fmt::{Display, Formatter};
use std::io::Error;
use std::num::ParseIntError;
use std::path::Path;
use std::sync::Arc;

#[derive(PartialEq, Debug, Clone)]
pub enum NameContext {
    Label,
    NodeName,
    PropertyName,
    UnitAddress,
}

#[derive(PartialEq, Debug, Clone)]
pub enum DiagnosticKind {
    UnexpectedEOF,
    Expected(Vec<TokenKind>),
    ExpectedName(NameContext),
    OddNumberOfBytestringElements,
    IntError(ParseIntError),
    NonDtsV1,
    NameTooLong(usize, NameContext),
    IllegalChar(char, NameContext),
    IllegalStart(char, NameContext),
    UnresolvedReference,
    PropertyReferencedByNode,
    NonStringInCompatible,
    PathCannotBeEmpty,
    PropertyAfterNode,
    UnbalancedParentheses,
    MisplacedDtsHeader,
    DuplicateDirective(CompilerDirective),
    ParserError(String),
    IOError(String),
    ErrorsInInclude,
    CyclicDependencyError(String),
}

pub enum SeverityLevel {
    Error,
    Warning,
    Hint,
}

impl Display for SeverityLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use SeverityLevel::*;
        match self {
            Error => write!(f, "error"),
            Warning => write!(f, "warning"),
            Hint => write!(f, "hint"),
        }
    }
}

impl DiagnosticKind {
    pub fn default_severity_level(&self) -> SeverityLevel {
        match self {
            DiagnosticKind::UnexpectedEOF => SeverityLevel::Error,
            DiagnosticKind::Expected(_) => SeverityLevel::Error,
            DiagnosticKind::ExpectedName(_) => SeverityLevel::Error,
            DiagnosticKind::OddNumberOfBytestringElements => SeverityLevel::Error,
            DiagnosticKind::IntError(_) => SeverityLevel::Error,
            DiagnosticKind::NonDtsV1 => SeverityLevel::Error,
            DiagnosticKind::NameTooLong(_, _) => SeverityLevel::Warning,
            DiagnosticKind::IllegalChar(_, _) => SeverityLevel::Error,
            DiagnosticKind::IllegalStart(_, _) => SeverityLevel::Error,
            DiagnosticKind::PathCannotBeEmpty => SeverityLevel::Error,
            DiagnosticKind::PropertyAfterNode => SeverityLevel::Error,
            DiagnosticKind::DuplicateDirective(_) => SeverityLevel::Warning,
            DiagnosticKind::UnbalancedParentheses => SeverityLevel::Error,
            DiagnosticKind::MisplacedDtsHeader => SeverityLevel::Error,
            DiagnosticKind::NonStringInCompatible => SeverityLevel::Warning,
            DiagnosticKind::UnresolvedReference => SeverityLevel::Error,
            DiagnosticKind::PropertyReferencedByNode => SeverityLevel::Error,
            DiagnosticKind::ParserError(_) => SeverityLevel::Error,
            DiagnosticKind::IOError(_) => SeverityLevel::Error,
            DiagnosticKind::ErrorsInInclude => SeverityLevel::Error,
            DiagnosticKind::CyclicDependencyError(..) => SeverityLevel::Error,
        }
    }
}

impl Display for NameContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NameContext::Label => write!(f, "label"),
            NameContext::NodeName => write!(f, "node name"),
            NameContext::PropertyName => write!(f, "property"),
            NameContext::UnitAddress => write!(f, "unit address"),
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

impl Display for DiagnosticKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagnosticKind::NonDtsV1 => {
                write!(f, "Files without the '/dts-v1/' Header are not supported")
            }
            DiagnosticKind::UnexpectedEOF => write!(f, "Unexpected End of File"),
            DiagnosticKind::ExpectedName(name) => write!(f, "Expected {name}"),
            DiagnosticKind::Expected(kinds) => {
                if kinds.len() == 1 {
                    write!(f, "Expected {}", kinds[0])
                } else {
                    write!(
                        f,
                        "Expected one of {}",
                        kinds.iter().map(|kind| format!("{kind}")).join(", ")
                    )
                }
            }
            DiagnosticKind::OddNumberOfBytestringElements => {
                write!(f, "Number of elements in byte string must be even")
            }
            DiagnosticKind::IntError(err) => write!(f, "{}", err),
            DiagnosticKind::NameTooLong(size, context) => write!(
                f,
                "{context} should only have 31 characters but has {size} characters",
            ),
            DiagnosticKind::IllegalChar(ch, context) => {
                write!(f, "Illegal char '{ch}' in {context}")
            }
            DiagnosticKind::IllegalStart(ch, context) => {
                write!(f, "{context} may not start with {ch}")
            }
            DiagnosticKind::UnresolvedReference => {
                write!(f, "Reference cannot be resolved")
            }
            DiagnosticKind::NonStringInCompatible => {
                write!(f, "compatible property should only contain strings")
            }
            DiagnosticKind::PathCannotBeEmpty => {
                write!(f, "Path cannot be empty")
            }
            DiagnosticKind::PropertyAfterNode => {
                write!(f, "Properties must be placed before nodes")
            }
            DiagnosticKind::DuplicateDirective(directive) => {
                write!(f, "Duplicate compiler directive {}", directive)
            }
            DiagnosticKind::UnbalancedParentheses => write!(f, "Unbalanced parentheses"),
            DiagnosticKind::MisplacedDtsHeader => {
                write!(f, "dts-v1 header must be placed on top of the file")
            }
            DiagnosticKind::PropertyReferencedByNode => {
                write!(f, "Reference points to a property, not a node")
            }
            DiagnosticKind::ParserError(str) => {
                write!(f, "{str}")
            }
            DiagnosticKind::IOError(msg) => {
                write!(f, "{msg}")
            }
            DiagnosticKind::ErrorsInInclude => {
                write!(f, "Included file contains non-recoverable errors")
            }
            DiagnosticKind::CyclicDependencyError(str) => {
                write!(f, "Cyclic import: {str}")
            }
        }
    }
}

impl From<Error> for DiagnosticKind {
    fn from(value: Error) -> Self {
        DiagnosticKind::IOError(format!("{value}"))
    }
}

impl<V> From<CyclicDependencyError<V>> for DiagnosticKind
where
    V: Display,
{
    fn from(value: CyclicDependencyError<V>) -> Self {
        let str = value
            .cycle()
            .iter()
            .map(|element| format!("{element}"))
            .join(" -> ");
        DiagnosticKind::CyclicDependencyError(str)
    }
}

impl From<ParseIntError> for DiagnosticKind {
    fn from(value: ParseIntError) -> Self {
        DiagnosticKind::IntError(value)
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct Diagnostic {
    kind: DiagnosticKind,
    span: Span,
    source: Arc<Path>,
}

impl Diagnostic {
    pub fn new(span: Span, source: Arc<Path>, kind: impl Into<DiagnosticKind>) -> Diagnostic {
        Diagnostic {
            kind: kind.into(),
            source,
            span,
        }
    }

    pub fn from_token(token: Token, kind: impl Into<DiagnosticKind>) -> Diagnostic {
        Diagnostic {
            kind: kind.into(),
            source: token.source(),
            span: token.span,
        }
    }

    pub fn kind(&self) -> &DiagnosticKind {
        &self.kind
    }

    pub fn default_severity(&self) -> SeverityLevel {
        self.kind().default_severity_level()
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
            diagnostic.default_severity(),
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
        write!(f, " {}", diagnostic.kind)?;
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
    use crate::dts::diagnostics::{Diagnostic, DiagnosticKind, DiagnosticPrinter};
    use crate::dts::lexer::TokenKind;
    use crate::dts::parser::Parser;
    use crate::dts::test::Code;
    use itertools::Itertools;

    #[test]
    fn display_missing_semicolon() {
        let code = Code::with_file_name("/ {}", "fname");
        let (_, diag) = code.parse(Parser::file);
        assert_eq!(
            diag,
            vec![Diagnostic::new(
                code.s1("}").end().as_span(),
                code.source(),
                DiagnosticKind::Expected(vec![TokenKind::Semicolon]),
            )]
        );
        let printer = DiagnosticPrinter {
            diagnostics: &diag,
            code: vec!["/ {}".into()],
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
            "fname",
        );
        let (_, diag) = code.parse(Parser::file);
        let printer = DiagnosticPrinter {
            diagnostics: &diag,
            code: code
                .code()
                .lines()
                .map(|line| line.to_string())
                .collect_vec(),
        };
        let formatter_err = "\
warning --> fname:4:13
  |
4 |             very-long-company,very-long-name;    
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ property should only have 31 characters but has 32 characters

".to_string();
        println!("{printer}");
        assert_eq!(formatter_err, format!("{printer}"));
    }
}
