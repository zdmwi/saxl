//! E004: a relation connects two elements outside the system.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E004"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a relation connects two outside elements"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (rel, ends) in cx.model.relations.iter().zip(&cx.resolved.relation_ends) {
            let Some((from, to)) = ends else { continue };
            let (f, t) = (cx.element(*from), cx.element(*to));
            if !f.kind.is_internal() && !t.kind.is_internal() {
                out.push(
                    Diagnostic::error(
                        "E004",
                        "relation connects two elements outside the system",
                        rel.span,
                        format!(
                            "`{}` ({}) and `{}` ({}) are both outside the system",
                            f.name.text,
                            f.kind.keyword(),
                            t.name.text,
                            t.kind.keyword()
                        ),
                    )
                    .with_help(
                        "a relation needs at least one service, store, or queue inside the system",
                    ),
                );
            }
        }
    }
}
