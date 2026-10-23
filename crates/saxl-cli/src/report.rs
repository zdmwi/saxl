//! Diagnostic output: human (via miette), JSON, and GitHub annotations.

use std::fmt;

use miette::{GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, SourceSpan};
use saxl_core::{ColumnUnit, Diagnostic, LineIndex, LocatedDiagnostic, Severity, SourceFile};
use serde::Serialize;

/// Adapts a `saxl_core::Diagnostic` to `miette::Diagnostic`.
struct Adapter<'a> {
    diag: &'a Diagnostic,
    source: &'a NamedSource<String>,
}

impl fmt::Debug for Adapter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.diag.fmt(f)
    }
}

impl fmt::Display for Adapter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.diag.message)
    }
}

impl std::error::Error for Adapter<'_> {}

impl miette::Diagnostic for Adapter<'_> {
    fn code<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        Some(Box::new(self.diag.code))
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(match self.diag.severity {
            Severity::Error => miette::Severity::Error,
            Severity::Warning => miette::Severity::Warning,
            Severity::Info => miette::Severity::Advice,
        })
    }

    fn help<'b>(&'b self) -> Option<Box<dyn fmt::Display + 'b>> {
        self.diag
            .help
            .as_ref()
            .map(|h| Box::new(h) as Box<dyn fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(self.diag.labels.iter().map(|l| {
            let span = SourceSpan::new(
                (l.span.start as usize).into(),
                l.span.end.saturating_sub(l.span.start) as usize,
            );
            let label = Some(l.message.clone());
            if l.primary {
                LabeledSpan::new_primary_with_span(label, span)
            } else {
                LabeledSpan::new_with_span(label, span)
            }
        })))
    }
}

/// Renders diagnostics for a terminal.
pub fn human(source: &SourceFile, diags: &[Diagnostic], color: bool) -> String {
    let theme = if color {
        GraphicalTheme::unicode()
    } else {
        GraphicalTheme::unicode_nocolor()
    };
    let handler = GraphicalReportHandler::new_themed(theme)
        .with_width(100)
        .with_links(false);
    let named = NamedSource::new(&source.name, source.text.clone());
    let mut out = String::new();
    for diag in diags {
        let adapter = Adapter {
            diag,
            source: &named,
        };
        if handler.render_report(&mut out, &adapter).is_err() {
            out.push_str(&format!(
                "{}[{}]: {}\n",
                severity_word(diag.severity),
                diag.code,
                diag.message
            ));
        }
        out.push('\n');
    }
    out
}

/// A one-line count such as `1 error, 2 warnings`.
pub fn summary(diags: &[Diagnostic]) -> String {
    let count = |s| diags.iter().filter(|d| d.severity == s).count();
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    format!(
        "{}, {}, {}",
        plural(count(Severity::Error), "error"),
        plural(count(Severity::Warning), "warning"),
        plural(count(Severity::Info), "info")
    )
}

fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

#[derive(Serialize)]
struct FileDiagnostic<'a> {
    file: &'a str,
    #[serde(flatten)]
    diagnostic: LocatedDiagnostic,
}

/// A JSON array of every diagnostic, each with its file name.
pub fn json(files: &[(&SourceFile, Vec<Diagnostic>)]) -> String {
    let mut out = Vec::new();
    for (source, diags) in files {
        let index = LineIndex::new(&source.text);
        for d in diags {
            out.push(FileDiagnostic {
                file: &source.name,
                diagnostic: d.locate(&source.text, &index, ColumnUnit::Chars),
            });
        }
    }
    serde_json::to_string_pretty(&out).unwrap_or_else(|_| "[]".to_owned())
}

/// GitHub Actions workflow commands, one line per diagnostic.
pub fn github(source: &SourceFile, diags: &[Diagnostic]) -> String {
    let index = LineIndex::new(&source.text);
    let mut out = String::new();
    for d in diags {
        let located = d.locate(&source.text, &index, ColumnUnit::Chars);
        let level = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "notice",
        };
        let mut message = d.message.clone();
        if let Some(primary) = located.labels.first() {
            message = format!("{message}: {}", primary.message);
        }
        if let Some(help) = &d.help {
            message = format!("{message}\nhelp: {help}");
        }
        let (line, col) = located
            .labels
            .first()
            .map_or((1, 1), |l| (l.range.start.line, l.range.start.col));
        out.push_str(&format!(
            "::{level} file={},line={line},col={col},title={}::{}\n",
            escape_property(&source.name),
            d.code,
            escape_data(&message)
        ));
    }
    out
}

fn escape_data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(s: &str) -> String {
    escape_data(s).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_escaping() {
        assert_eq!(escape_data("50%\nnext"), "50%25%0Anext");
        assert_eq!(escape_property("C:\\a,b"), "C%3A\\a%2Cb");
    }

    #[test]
    fn summary_pluralizes() {
        assert_eq!(summary(&[]), "0 errors, 0 warnings, 0 infos");
    }
}
