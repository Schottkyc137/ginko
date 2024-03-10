use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

/// Device-tree source file types. Known types are:
///
/// * `FileType::DtSource`: Plain device-tree source files. Usually ends with `.dts`
/// * `FileType::DtSourceInclude`: Device-tree source include files.
///     Usually included by device-tree files and ends with `.dtsi`
/// * `FileType::DtSourceOverlay`: Device-tree source overlay files.
///     Usually built as standalone and uses references to define where an overlay should be applied.
///     Usually, these files end with `.dtso`
/// * `FileType::Unknown`: Unknown file type.
#[derive(Clone, Copy, Eq, PartialEq, Default, Debug)]
pub enum FileType {
    DtSource,
    DtSourceInclude,
    DtSourceOverlay,
    #[default]
    Unknown,
}

impl FileType {
    /// Guesses the file type from the ending of a file, excluding the dot.
    ///
    /// # Example usage:
    /// ```
    /// use ginko::dts::FileType;
    ///
    /// let file_name = "some_tree.dts";
    /// let ending = file_name
    ///     .split(".")
    ///     .last()
    ///     .map(FileType::from_file_ending)
    ///     .unwrap_or_default();
    /// assert_eq!(ending, FileType::DtSource);
    ///
    /// let other_file = "some_file.txt";
    /// let ending = other_file
    ///     .split(".")
    ///     .last()
    ///     .map(FileType::from_file_ending)
    ///     .unwrap_or_default();
    /// assert_eq!(ending, FileType::Unknown);
    /// ```
    pub fn from_file_ending(ending: &str) -> FileType {
        match ending {
            "dts" => FileType::DtSource,
            "dtsi" => FileType::DtSourceInclude,
            "dtso" => FileType::DtSourceOverlay,
            _ => FileType::Unknown,
        }
    }
}

/// A source position, defined by its zero-based line offset and zero-based character offset.
/// This is intentionally equivalent to the position defined by the LSP standard
/// to make conversions easier.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Position {
    line: u32,
    character: u32,
}

impl Display for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.character)
    }
}

impl Debug for Position {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl Position {
    pub fn line(&self) -> u32 {
        self.line
    }

    pub fn character(&self) -> u32 {
        self.character
    }

    pub fn zero() -> Position {
        Position::new(0, 0)
    }

    pub fn new(line: u32, col: u32) -> Position {
        Position {
            line,
            character: col,
        }
    }

    /// Returns a `Span` that is formed by going from this position to another position.
    ///
    /// # Exceptions
    /// The given position may not be greater than this position.
    /// If this is the case in debug builds, this function will panic. However, since most
    /// applications can gracefully handle this condition, this function will not panic
    /// in production builds.
    pub fn to(&self, other: Position) -> Span {
        debug_assert!(other > *self, "Position {other} is past position {self}");
        Span::new(*self, other)
    }

    #[cfg(test)]
    pub fn char_to(&self, col: u32) -> Span {
        Span::new(*self, Position::new(self.line, col))
    }

    pub fn offset_by_char(&self, count: i32) -> Position {
        Position::new(
            self.line,
            self.character.checked_add_signed(count).expect(
                &format!(
                    "[offset_by_char] Illegal position reached. self: {}, count: {}",
                    self, count
                )[..],
            ),
        )
    }

    /// Returns the zero-length span that is formed by this position repeated
    ///```
    /// use ginko::dts::{Position, Span};
    ///
    /// let pos = Position::new(3, 4);
    /// let span = pos.as_span();
    /// assert_eq!(span, Span::new(Position::new(3, 4), Position::new(3, 4)));
    ///```
    pub fn as_span(&self) -> Span {
        Span::new(*self, *self)
    }

    /// Returns a span with length 1
    ///```
    /// use ginko::dts::{Position, Span};
    ///
    /// let pos = Position::new(3, 4);
    /// let span = pos.as_char_span();
    /// assert_eq!(span, Span::new(Position::new(3, 4), Position::new(3, 5)));
    ///```
    pub fn as_char_span(&self) -> Span {
        Span::new(*self, self.offset_by_char(1))
    }
}

/// A span in a source text. Defined by it's starting position and end position
/// where the start is inclusive but the end is not.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Span {
    start: Position,
    end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Span {
        Span { start, end }
    }

    pub fn start(&self) -> Position {
        self.start
    }

    pub fn end(&self) -> Position {
        self.end
    }

    pub fn contains(&self, position: &Position) -> bool {
        self.start <= *position && self.end > *position
    }

    pub fn extend_start(&self, magnitude: i32) -> Span {
        Span {
            start: self.start().offset_by_char(magnitude),
            end: self.end(),
        }
    }

    #[cfg(test)]
    #[allow(unused)]
    pub fn extend_end(&self, magnitude: i32) -> Span {
        Span {
            start: self.start(),
            end: self.end().offset_by_char(magnitude),
        }
    }
}

pub trait HasSpan {
    fn span(&self) -> Span;

    fn start(&self) -> Position {
        self.span().start()
    }

    fn end(&self) -> Position {
        self.span().end()
    }
}

impl HasSpan for Span {
    fn span(&self) -> Span {
        *self
    }
}

pub trait HasSource {
    fn source(&self) -> Arc<str>;
}
