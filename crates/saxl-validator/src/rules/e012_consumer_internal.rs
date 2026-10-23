//! E012: a consumer calls something that is not an `api`.

use saxl_core::{Diagnostic, ElementKind, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E012"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a consumer calls something that is not an `api`"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (rel, ends) in cx.model.relations.iter().zip(&cx.resolved.relation_ends) {
            let Some((from, to)) = ends else { continue };
            let (f, t) = (cx.element(*from), cx.element(*to));
            // Calls to other outside elements are E004's.
            if f.kind != ElementKind::Consumer || t.kind != ElementKind::Module {
                continue;
            }
            out.push(
                Diagnostic::error(
                    "E012",
                    format!("consumer `{}` calls an internal module", f.name.text),
                    rel.span,
                    format!("`{}` is not part of the public API", t.name.text),
                )
                .with_help(format!(
                    "call an api that uses `{0}`, or declare `{0}` as `api`",
                    t.name.text
                )),
            );
        }
    }
}
