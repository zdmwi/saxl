//! E013: an `impl` statement names the wrong kind of element.

use saxl_core::{Diagnostic, ElementKind, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E013"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "an `impl` statement names the wrong kind of element"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        // In a system, E015 reports `impl` itself.
        if !cx.is_library() {
            return;
        }
        for (stmt, ends) in cx.model.impls.iter().zip(&cx.resolved.impl_ends) {
            let Some((implementor, hook)) = ends else {
                continue;
            };
            let (i, h) = (cx.element(*implementor), cx.element(*hook));
            if h.kind != ElementKind::Hook {
                out.push(
                    Diagnostic::error(
                        "E013",
                        format!("`{}` implements `{}`, which is not a hook", i.name.text, h.name.text),
                        stmt.hook.span,
                        format!("`{}` is {} {}", h.name.text, h.kind.article(), h.kind.keyword()),
                    )
                    .with_help(format!(
                        "only hooks can be implemented; declare `{}` as `hook`, or remove this line",
                        h.name.text
                    )),
                );
            }
            if !matches!(
                i.kind,
                ElementKind::Api | ElementKind::Module | ElementKind::Consumer
            ) {
                out.push(
                    Diagnostic::error(
                        "E013",
                        format!("`{}` can't implement a hook", i.name.text),
                        stmt.implementor.span,
                        format!(
                            "`{}` is {} {}",
                            i.name.text,
                            i.kind.article(),
                            i.kind.keyword()
                        ),
                    )
                    .with_help("a hook is implemented by an api, a module, or a consumer"),
                );
            }
        }
    }
}
