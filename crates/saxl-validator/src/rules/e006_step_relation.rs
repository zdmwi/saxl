//! E006: a scenario step has no matching declared relation.

use saxl_core::{Diagnostic, Severity, StepArrow, Style};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E006"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a scenario step has no matching relation"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for scenario in cx.scenarios() {
            super::for_each_message(&scenario.steps, &mut |hops, arrows, span| {
                for hop in super::element_hops(cx.resolved, hops, arrows) {
                    if hop.arrow == StepArrow::Reply
                        || cx.has_relation(hop.from, hop.to, Style::Sync)
                        || cx.has_relation(hop.from, hop.to, Style::Async)
                    {
                        continue;
                    }
                    let rel = format!(
                        "{} {} {}",
                        cx.element(hop.from).name.text,
                        hop.arrow.arrow(),
                        cx.element(hop.to).name.text
                    );
                    out.push(
                        Diagnostic::error(
                            "E006",
                            "scenario step has no matching relation",
                            span,
                            format!("no relation `{rel}` is declared"),
                        )
                        .with_help(format!(
                            "add `{rel}` to the relations, or route this step through a service that has one"
                        )),
                    );
                }
            });
        }
    }
}
