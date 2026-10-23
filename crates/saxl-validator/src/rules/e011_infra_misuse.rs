//! E011: an infrastructure node is used where an element is required.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E011"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "an infrastructure node is used in a relation, data placement, or deployment"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        super::resolution_diagnostics(cx, "E011", out);
    }
}
