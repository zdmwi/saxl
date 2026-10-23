//! E003: there must be exactly one `system` statement.

use saxl_core::{Diagnostic, FileId, Severity, Span};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E003"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "there must be exactly one `system` statement"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        match cx.model.roots.as_slice() {
            [] => {
                let m = cx.model;
                let file = m
                    .elements
                    .first()
                    .map(|e| e.span.file)
                    .or_else(|| m.relations.first().map(|r| r.span.file))
                    .or_else(|| m.regions.first().map(|r| r.span.file))
                    .or_else(|| m.scenarios.first().map(|s| s.span.file))
                    .or_else(|| m.allows.first().map(|a| a.span.file))
                    .unwrap_or(FileId(0));
                out.push(
                    Diagnostic::error(
                        "E003",
                        "no `system` statement",
                        Span::new(file, 0, 0),
                        "file starts here",
                    )
                    .with_help(
                        "add `system <Name> \"description\"` to name the system being described",
                    ),
                );
            }
            [_] => {}
            [first, rest @ ..] => {
                let first_kw = first.kind.keyword();
                for root in rest {
                    let kw = root.kind.keyword();
                    let (message, help) = if root.kind == first.kind {
                        (
                            format!("more than one `{kw}` statement"),
                            format!("describe one {kw} per file"),
                        )
                    } else {
                        (
                            "both a `system` and a `library` statement".to_owned(),
                            "describe the system and the library in separate files".to_owned(),
                        )
                    };
                    out.push(
                        Diagnostic::error("E003", message, root.span, format!("second `{kw}`"))
                            .with_secondary(first.span, format!("first `{first_kw}` here"))
                            .with_help(help),
                    );
                }
            }
        }
    }
}
