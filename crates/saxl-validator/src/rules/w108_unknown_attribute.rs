//! W108: unknown deployment attribute.

use saxl_core::{Attribute, Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W108"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "unknown deployment attribute"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let workloads = cx
            .model
            .regions
            .iter()
            .flat_map(|r| &r.groups)
            .flat_map(|g| &g.workloads);
        for attr in workloads.flat_map(|w| &w.attributes) {
            if let Attribute::Unknown { text, span } = attr {
                out.push(
                    Diagnostic::new(
                        "W108",
                        Severity::Warning,
                        format!("unknown deployment attribute `{text}`"),
                        *span,
                        "not a known attribute",
                    )
                    .with_help(
                        "known attributes are `xN` (replicas), `autoscale`, and `primary+replica`",
                    ),
                );
            }
        }
    }
}
