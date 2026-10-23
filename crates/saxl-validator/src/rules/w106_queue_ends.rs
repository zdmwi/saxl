//! W106: a queue has no producer or no consumer.

use petgraph::Direction;
use saxl_core::{Diagnostic, ElementKind, Severity};

use super::Context;
use crate::resolve::ElementId;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W106"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "a queue has no producer or no consumer"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (i, element) in cx.model.elements.iter().enumerate() {
            if element.kind != ElementKind::Queue {
                continue;
            }
            let node = ElementId(u32::try_from(i).unwrap_or(u32::MAX)).node();
            let graph = &cx.resolved.graph;
            let producer = graph
                .neighbors_directed(node, Direction::Incoming)
                .next()
                .is_some();
            let consumer = graph
                .neighbors_directed(node, Direction::Outgoing)
                .next()
                .is_some();
            let missing = match (producer, consumer) {
                (true, true) => continue,
                (false, true) => "no producer",
                (true, false) => "no consumer",
                (false, false) => "no producer or consumer",
            };
            out.push(
                Diagnostic::new(
                    "W106",
                    Severity::Warning,
                    format!("queue `{}` has {missing}", element.name.text),
                    element.span,
                    missing,
                )
                .with_help(format!(
                    "add a relation into `{0}` (`Producer ~> {0}`) and one out of it (`{0} ~> Consumer`)",
                    element.name.text
                )),
            );
        }
    }
}
