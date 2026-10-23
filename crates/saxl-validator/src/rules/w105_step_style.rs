//! W105: a step's style differs from its relation's style.

use saxl_core::{Diagnostic, Severity, StepArrow, Style};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W105"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "a step's style differs from its relation's style"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for scenario in cx.scenarios() {
            super::for_each_message(&scenario.steps, &mut |hops, arrows, span| {
                for hop in super::element_hops(cx.resolved, hops, arrows) {
                    let (style, other) = match hop.arrow {
                        StepArrow::Call => (Style::Sync, Style::Async),
                        StepArrow::Async => (Style::Async, Style::Sync),
                        StepArrow::Reply => continue,
                    };
                    if cx.has_relation(hop.from, hop.to, style)
                        || !cx.has_relation(hop.from, hop.to, other)
                    {
                        continue;
                    }
                    let (from, to) = (
                        &cx.element(hop.from).name.text,
                        &cx.element(hop.to).name.text,
                    );
                    out.push(
                        Diagnostic::new(
                            "W105",
                            Severity::Warning,
                            "step style differs from its relation",
                            span,
                            format!(
                                "step is `{from} {} {to}`, but the relation is `{from} {} {to}`",
                                style.arrow(),
                                other.arrow()
                            ),
                        )
                        .with_help(format!(
                            "use `{}` in the step, or change the relation",
                            other.arrow()
                        )),
                    );
                }
            });
        }
    }
}
