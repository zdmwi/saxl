//! W109: a module depends on one gated behind a feature it is not gated on.

use saxl_core::{Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W109"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "a module depends on one gated behind a feature it is not gated on"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !cx.is_library() {
            return;
        }
        let publications = &cx.resolved.publications;
        for (rel, ends) in cx.model.relations.iter().zip(&cx.resolved.relation_ends) {
            let Some((from, to)) = ends else { continue };
            // Compare where both ends ship in the same package.
            let leak = publications[from.index()].iter().find_map(|f| {
                publications[to.index()]
                    .iter()
                    .find(|t| {
                        (t.registry, t.package) == (f.registry, f.package)
                            && t.feature.is_some()
                            && t.feature != f.feature
                    })
                    .map(|t| (f, t))
            });
            let Some((f, t)) = leak else { continue };
            let (from_name, to_name) = (&cx.element(*from).name.text, &cx.element(*to).name.text);
            let needed = t.feature.as_deref().unwrap_or_default();
            let message = match &f.feature {
                None => format!(
                    "always-built `{from_name}` depends on `{to_name}`, which needs feature `{needed}`"
                ),
                Some(own) => format!(
                    "`{from_name}` (feature `{own}`) depends on `{to_name}`, which needs feature `{needed}`"
                ),
            };
            out.push(
                Diagnostic::new(
                    "W109",
                    Severity::Warning,
                    message,
                    rel.span,
                    format!("`{to_name}` is only built with feature `{needed}`"),
                )
                .with_help(format!(
                    "call a hook that `{to_name}` implements instead, or gate `{from_name}` behind `{needed}` too"
                )),
            );
        }
    }
}
