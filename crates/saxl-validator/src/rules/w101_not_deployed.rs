//! W101: an internal element is not deployed in any region.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W101"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "an internal element is not deployed in any region"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (element, deployments) in cx.model.elements.iter().zip(&cx.resolved.deployments) {
            if element.kind.is_internal() && deployments.is_empty() {
                out.push(
                    Diagnostic::new(
                        "W101",
                        Severity::Warning,
                        format!("`{}` is not deployed in any region", element.name.text),
                        element.span,
                        "nowhere to run",
                    )
                    .with_help("add it to a cluster or managed group in a `deploy region` block"),
                );
            }
        }
    }
}
