//! E007: a reply has no matching open call.

use std::collections::BTreeSet;

use saxl_core::{Diagnostic, Severity, Step, StepArrow};

use super::Context;
use crate::resolve::ElementId;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E007"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a reply has no matching open call"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for scenario in cx.scenarios() {
            let mut walker = Walker {
                cx,
                next_id: 0,
                out,
            };
            walker.walk(&scenario.steps, &mut Vec::new());
        }
    }
}

/// An open call. `id` tells copies of the same call apart from new ones.
#[derive(Clone, Copy)]
struct Open {
    id: usize,
    caller: ElementId,
    callee: ElementId,
}

struct Walker<'a, 'b> {
    cx: &'a Context<'a>,
    next_id: usize,
    out: &'b mut Vec<Diagnostic>,
}

impl Walker<'_, '_> {
    fn walk(&mut self, steps: &[Step], stack: &mut Vec<Open>) {
        for step in steps {
            match step {
                Step::Message {
                    hops, arrows, span, ..
                } => {
                    for hop in super::element_hops(self.cx.resolved, hops, arrows) {
                        match hop.arrow {
                            StepArrow::Call => {
                                stack.push(Open {
                                    id: self.next_id,
                                    caller: hop.from,
                                    callee: hop.to,
                                });
                                self.next_id += 1;
                            }
                            StepArrow::Async => {}
                            StepArrow::Reply => {
                                // A reply from B to A closes the innermost open call A -> B
                                // and every call opened after it.
                                match stack
                                    .iter()
                                    .rposition(|o| o.caller == hop.to && o.callee == hop.from)
                                {
                                    Some(pos) => stack.truncate(pos),
                                    None => {
                                        let from = &self.cx.element(hop.from).name.text;
                                        let to = &self.cx.element(hop.to).name.text;
                                        self.out.push(
                                            Diagnostic::error(
                                                "E007",
                                                "reply has no matching open call",
                                                *span,
                                                format!("no open call `{to} -> {from}` to reply to"),
                                            )
                                            .with_help(format!(
                                                "a `{from} --> {to}` reply needs an earlier `{to} -> {from}` call in the same scenario"
                                            )),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Step::Parallel { steps, .. } => {
                    // Each branch starts from the current stack. An outer call is
                    // closed if any branch closed it; calls opened in branches are dropped.
                    let base = stack.clone();
                    let mut closed = BTreeSet::new();
                    for branch in steps {
                        let mut local = base.clone();
                        self.walk(std::slice::from_ref(branch), &mut local);
                        for open in &base {
                            if !local.iter().any(|o| o.id == open.id) {
                                closed.insert(open.id);
                            }
                        }
                    }
                    *stack = base
                        .into_iter()
                        .filter(|o| !closed.contains(&o.id))
                        .collect();
                }
            }
        }
    }
}
