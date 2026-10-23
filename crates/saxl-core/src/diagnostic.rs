use serde::Serialize;

use crate::{LineIndex, Position, Span};

/// A stable diagnostic code such as `"E006"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Code(pub &'static str);

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "ts", ts(rename_all = "lowercase"))]
pub enum Severity {
    /// Blocks rendering.
    Error,
    /// A likely problem. Does not block rendering.
    Warning,
    /// A note. Does not block rendering.
    Info,
}

/// A span with a short message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Label {
    /// The labeled source range.
    pub span: Span,
    /// Short text shown next to the range.
    pub message: String,
    /// Whether this is the diagnostic's main location.
    pub primary: bool,
}

impl Label {
    /// A primary label.
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            primary: true,
        }
    }

    /// A secondary label.
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            primary: false,
        }
    }
}

/// A problem reported by any stage of the toolchain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Diagnostic {
    /// Stable code, for example `E006`.
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub code: Code,
    /// Severity.
    pub severity: Severity,
    /// One-line message.
    pub message: String,
    /// Labels. Exactly one is primary.
    pub labels: Vec<Label>,
    /// Optional help line.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Creates a diagnostic with a primary label.
    pub fn new(
        code: &'static str,
        severity: Severity,
        message: impl Into<String>,
        span: Span,
        label: impl Into<String>,
    ) -> Self {
        Self {
            code: Code(code),
            severity,
            message: message.into(),
            labels: vec![Label::primary(span, label)],
            help: None,
        }
    }

    /// Creates an error with a primary label.
    pub fn error(
        code: &'static str,
        message: impl Into<String>,
        span: Span,
        label: impl Into<String>,
    ) -> Self {
        Self::new(code, Severity::Error, message, span, label)
    }

    /// Adds a secondary label.
    pub fn with_secondary(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::secondary(span, message));
        self
    }

    /// Sets the help line.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// The primary label, if any.
    pub fn primary(&self) -> Option<&Label> {
        self.labels.iter().find(|l| l.primary)
    }

    /// Start offset of the primary label, used for sorting.
    pub fn sort_key(&self) -> u32 {
        self.primary().map_or(0, |l| l.span.start)
    }
}

/// How columns are counted in a [`LocatedDiagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnUnit {
    /// Unicode scalar values, for terminals.
    Chars,
    /// UTF-16 code units, for browser editors.
    Utf16,
}

/// A byte range without its file, as written in JSON output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ByteRange {
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset one past the last byte.
    pub end: u32,
}

/// A start and end position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Range {
    /// Where the range starts.
    pub start: Position,
    /// Where the range ends (exclusive).
    pub end: Position,
}

/// A label with its line and column range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct LocatedLabel {
    /// Whether this is the diagnostic's main location.
    pub primary: bool,
    /// Short text shown next to the range.
    pub message: String,
    /// Byte offsets.
    pub span: ByteRange,
    /// 1-based lines and columns.
    pub range: Range,
}

/// The JSON form of a diagnostic, shared by `saxl check --format json` and the
/// playground. Labels carry line and column ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct LocatedDiagnostic {
    /// Stable code, for example `E006`.
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub code: Code,
    /// Severity.
    pub severity: Severity,
    /// One-line message.
    pub message: String,
    /// Labels, primary first.
    pub labels: Vec<LocatedLabel>,
    /// Optional help line.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Resolves every label to lines and columns in `text`.
    pub fn locate(&self, text: &str, index: &LineIndex, unit: ColumnUnit) -> LocatedDiagnostic {
        let pos = |offset| match unit {
            ColumnUnit::Chars => index.position(text, offset),
            ColumnUnit::Utf16 => index.position_utf16(text, offset),
        };
        let mut labels: Vec<_> = self
            .labels
            .iter()
            .map(|l| LocatedLabel {
                primary: l.primary,
                message: l.message.clone(),
                span: ByteRange {
                    start: l.span.start,
                    end: l.span.end,
                },
                range: Range {
                    start: pos(l.span.start),
                    end: pos(l.span.end),
                },
            })
            .collect();
        labels.sort_by_key(|l| !l.primary);
        LocatedDiagnostic {
            code: self.code,
            severity: self.severity,
            message: self.message.clone(),
            labels,
            help: self.help.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileId;

    #[test]
    fn locate_counts_columns_in_both_units() {
        let text = "a𝄞 bad\n";
        let start = text.find("bad").unwrap_or_default() as u32;
        let d = Diagnostic::error("E002", "m", Span::new(FileId(0), start, start + 3), "here");
        let idx = LineIndex::new(text);
        let chars = d.locate(text, &idx, ColumnUnit::Chars);
        let utf16 = d.locate(text, &idx, ColumnUnit::Utf16);
        assert_eq!(chars.labels[0].range.start, Position { line: 1, col: 4 });
        assert_eq!(utf16.labels[0].range.start, Position { line: 1, col: 5 });
        assert_eq!(
            chars.labels[0].span,
            ByteRange {
                start,
                end: start + 3
            }
        );
    }
}
