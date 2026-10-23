use serde::Serialize;

use crate::Span;

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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
