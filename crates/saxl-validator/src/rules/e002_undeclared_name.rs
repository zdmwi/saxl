//! E002: a reference names nothing that is declared.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E002"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a reference names nothing that is declared"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        super::resolution_diagnostics(cx, "E002", out);
    }
}
