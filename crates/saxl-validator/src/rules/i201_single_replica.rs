//! I201: a single-replica service takes part in synchronous calls.

use std::collections::BTreeSet;

use saxl_core::{Diagnostic, ElementKind, Severity, StepArrow};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "I201"
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn summary(&self) -> &'static str {
        "a single-replica service takes part in synchronous calls"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let mut in_calls = BTreeSet::new();
        for scenario in cx.scenarios() {
            super::for_each_message(&scenario.steps, &mut |hops, arrows, _| {
                for hop in super::element_hops(cx.resolved, hops, arrows) {
                    if hop.arrow == StepArrow::Call {
                        in_calls.insert(hop.from);
                        in_calls.insert(hop.to);
                    }
                }
            });
        }
        for id in in_calls {
            let element = cx.element(id);
            if element.kind != ElementKind::Service {
                continue;
            }
            let single = cx.resolved.deployments[id.index()]
                .iter()
                .find(|d| d.replicas == 1 && !d.autoscale);
            if let Some(d) = single {
                out.push(
                    Diagnostic::new(
                        "I201",
                        Severity::Info,
                        format!("service `{}` runs one replica but serves synchronous calls", element.name.text),
                        d.span,
                        "single replica",
                    )
                    .with_help("callers wait on it, so it is a single point of failure; add replicas (`x2`) or `autoscale`"),
                );
            }
        }
    }
}
