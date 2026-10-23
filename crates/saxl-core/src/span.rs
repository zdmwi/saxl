use serde::{Deserialize, Serialize};

/// Identifies a source file within one compilation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FileId(pub u32);

/// A byte range in a source file. `end` is exclusive.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Span {
    /// The file the range belongs to.
    pub file: FileId,
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset one past the last byte.
    pub end: u32,
}

impl Span {
    /// Creates a span in `file` covering `start..end`.
    pub fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// A named source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    /// The id spans use to refer to this file.
    pub id: FileId,
    /// Display name, usually the path.
    pub name: String,
    /// The full text.
    pub text: String,
}

/// A 1-based line and column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column, in characters or UTF-16 code units depending on the query.
    pub col: u32,
}

/// Maps byte offsets to line and column positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    line_starts: Vec<u32>,
}

impl LineIndex {
    /// Indexes the line starts of `text`.
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(to_u32(i + 1));
            }
        }
        Self { line_starts }
    }

    /// Position of `offset`, with the column counted in Unicode scalar values.
    pub fn position(&self, text: &str, offset: u32) -> Position {
        self.position_with(text, offset, |s| s.chars().count())
    }

    /// Position of `offset`, with the column counted in UTF-16 code units.
    pub fn position_utf16(&self, text: &str, offset: u32) -> Position {
        self.position_with(text, offset, |s| s.encode_utf16().count())
    }

    fn position_with(&self, text: &str, offset: u32, width: impl Fn(&str) -> usize) -> Position {
        let offset = floor_char_boundary(text, offset as usize);
        let line = self
            .line_starts
            .partition_point(|&start| start as usize <= offset)
            .saturating_sub(1);
        let start = self.line_starts.get(line).copied().unwrap_or(0) as usize;
        let col = text.get(start..offset).map_or(0, width);
        Position {
            line: to_u32(line + 1),
            col: to_u32(col + 1),
        }
    }
}

fn floor_char_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_positions() {
        let text = "ab\ncd\n";
        let idx = LineIndex::new(text);
        assert_eq!(idx.position(text, 0), Position { line: 1, col: 1 });
        assert_eq!(idx.position(text, 2), Position { line: 1, col: 3 });
        assert_eq!(idx.position(text, 3), Position { line: 2, col: 1 });
        assert_eq!(idx.position(text, 6), Position { line: 3, col: 1 });
        assert_eq!(idx.position(text, 99), Position { line: 3, col: 1 });
    }

    #[test]
    fn multibyte_columns() {
        // `→` is 3 bytes in UTF-8, 1 char, 1 UTF-16 unit. `𝄞` is 4 bytes, 1 char, 2 UTF-16 units.
        let text = "a→b𝄞c";
        let idx = LineIndex::new(text);
        let c = text.find('c').unwrap_or_default() as u32;
        assert_eq!(idx.position(text, c), Position { line: 1, col: 5 });
        assert_eq!(idx.position_utf16(text, c), Position { line: 1, col: 6 });
        // An offset inside `→` snaps back to its start.
        assert_eq!(idx.position(text, 2), Position { line: 1, col: 2 });
    }
}
