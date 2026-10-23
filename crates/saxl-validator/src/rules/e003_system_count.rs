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
        match cx.model.systems.as_slice() {
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
                for system in rest {
                    out.push(
                        Diagnostic::error(
                            "E003",
                            "more than one `system` statement",
                            system.span,
                            "second `system`",
                        )
                        .with_secondary(first.span, "first `system` here")
                        .with_help("describe one system per file"),
                    );
                }
            }
        }
    }
}
