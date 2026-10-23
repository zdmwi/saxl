//! E001: a name is declared twice.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E001"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a name is declared twice"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        super::resolution_diagnostics(cx, "E001", out);
    }
}
