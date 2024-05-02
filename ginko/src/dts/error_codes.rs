use crate::dts::Severity;
use enum_map::{enum_map, Enum, EnumMap};
use std::ops::{Index, IndexMut};
use strum::{AsRefStr, EnumString};

#[derive(PartialEq, Debug, Clone, EnumString, AsRefStr, Enum)]
#[strum(serialize_all = "snake_case")]
pub enum ErrorCode {
    UnexpectedEOF,
    Expected,
    ExpectedName,
    OddNumberOfBytestringElements,
    IntError,
    NonDtsV1,
    NameTooLong,
    IllegalChar,
    IllegalStart,
    UnresolvedReference,
    PropertyReferencedByNode,
    NonStringInCompatible,
    PathCannotBeEmpty,
    PropertyAfterNode,
    UnbalancedParentheses,
    MisplacedDtsHeader,
    DuplicateDirective,
    ParserError,
    IOError,
    ErrorsInInclude,
    CyclicDependencyError,
}

/// The `SeverityMap` maps error codes to severities.
///
/// Implementations for `Index` and `IndexMut` are provided, so elements within the map can
/// be accessed using the `[]` operator.
/// The value returned by indexing into the severity map has the following meaning:
/// * If the value is `Some(Severity)`,
///   a diagnostic with the given error code should be displayed with that severity
/// * If the value is `None`, a diagnostic with that severity should not be displayed
#[derive(Clone, PartialEq, Eq, Debug, Copy)]
pub struct SeverityMap {
    // Using an `EnumMap` ensures that each error code is mapped to exactly one severity.
    // Additionally, this allows efficient implementation using an array internally.
    inner: EnumMap<ErrorCode, Severity>,
}

impl Default for SeverityMap {
    fn default() -> Self {
        use ErrorCode::*;
        let map = enum_map! {
            UnexpectedEOF
            | Expected
            | ExpectedName
            | OddNumberOfBytestringElements
            | IntError
            | NonDtsV1
            | IllegalChar
            | IllegalStart
            | UnresolvedReference
            | PropertyReferencedByNode
            | PathCannotBeEmpty
            | PropertyAfterNode
            | UnbalancedParentheses
            | MisplacedDtsHeader
            | ParserError
            | IOError
            | ErrorsInInclude
            | CyclicDependencyError => Severity::Error,
            NameTooLong
            | NonStringInCompatible
            | DuplicateDirective => Severity::Warning
        };
        SeverityMap { inner: map }
    }
}

impl Index<ErrorCode> for SeverityMap {
    type Output = Severity;

    fn index(&self, key: ErrorCode) -> &Self::Output {
        self.inner.index(key)
    }
}

impl IndexMut<ErrorCode> for SeverityMap {
    fn index_mut(&mut self, key: ErrorCode) -> &mut Self::Output {
        self.inner.index_mut(key)
    }
}
