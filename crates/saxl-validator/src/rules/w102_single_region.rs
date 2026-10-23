//! W102: every workload is in a single region.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W102"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "every workload is in a single region"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if cx.is_library() {
            return;
        }
        let mut used = cx
            .model
            .regions
            .iter()
            .filter(|r| r.groups.iter().any(|g| !g.workloads.is_empty()));
        if let (Some(region), None) = (used.next(), used.next()) {
            out.push(
                Diagnostic::new(
                    "W102",
                    Severity::Warning,
                    "every workload is in a single region",
                    region.name.span,
                    "a regional outage takes the whole system down",
                )
                .with_help("deploy to a second region, or `allow W102` with a reason"),
            );
        }
    }
}
