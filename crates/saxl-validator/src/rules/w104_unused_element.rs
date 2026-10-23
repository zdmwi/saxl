//! W104: an element takes part in nothing.

use saxl_core::{Diagnostic, NameRef, Placement, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W104"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "an element takes part in no relation, data placement, or scenario"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let mut used = vec![false; cx.model.elements.len()];
        for (f, t) in cx.resolved.relation_ends.iter().flatten() {
            used[f.index()] = true;
            used[t.index()] = true;
        }
        let mut mark = |r: &NameRef| {
            if let Some(id) = cx.resolved.element(&r.text) {
                used[id.index()] = true;
            }
        };
        for data in &cx.model.data {
            match &data.placement {
                Placement::In { store, cached } => {
                    mark(store);
                    cached.iter().for_each(&mut mark);
                }
                Placement::Via { queue, store } => {
                    mark(queue);
                    mark(store);
                }
            }
        }
        for scenario in &cx.model.scenarios {
            super::for_each_message(&scenario.steps, &mut |hops, _, _| {
                hops.iter().for_each(&mut mark)
            });
        }
        for (element, used) in cx.model.elements.iter().zip(used) {
            if !used {
                out.push(
                    Diagnostic::new(
                        "W104",
                        Severity::Warning,
                        format!("`{}` is not used", element.name.text),
                        element.span,
                        "in no relation, data placement, or scenario",
                    )
                    .with_help("connect it with a relation, or remove it"),
                );
            }
        }
    }
}
