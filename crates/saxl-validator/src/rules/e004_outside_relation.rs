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
                let (root, inside) = if cx.is_library() {
                    ("library", "api or module")
                } else {
                    ("system", "service, store, or queue")
                };
                out.push(
                    Diagnostic::error(
                        "E004",
                        format!("relation connects two elements outside the {root}"),
                        rel.span,
                        format!(
                            "`{}` ({}) and `{}` ({}) are both outside the {root}",
                            f.name.text,
                            f.kind.keyword(),
                            t.name.text,
                            t.kind.keyword()
                        ),
                    )
                    .with_help(format!(
                        "a relation needs at least one {inside} inside the {root}"
                    )),
                );
            }
        }
    }
}
