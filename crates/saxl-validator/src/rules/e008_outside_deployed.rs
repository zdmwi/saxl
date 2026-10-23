//! E008: an actor or external appears in a deployment.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E008"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "an actor or external appears in a deployment"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (element, deployments) in cx.model.elements.iter().zip(&cx.resolved.deployments) {
            if element.kind.is_internal() {
                continue;
            }
            for d in deployments {
                out.push(
                    Diagnostic::error(
                        "E008",
                        format!(
                            "`{}` is an {} and can't be deployed",
                            element.name.text,
                            element.kind.keyword()
                        ),
                        d.span,
                        format!("{}s run outside the system", element.kind.keyword()),
                    )
                    .with_help("only services, stores, and queues are deployed"),
                );
            }
        }
    }
}
