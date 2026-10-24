//! I202: a hook has no implementation inside the library.

use saxl_core::{Diagnostic, ElementKind, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "I202"
    }

    fn severity(&self) -> Severity {
        Severity::Info
    }

    fn summary(&self) -> &'static str {
        "a hook has no implementation inside the library"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (i, element) in cx.model.elements.iter().enumerate() {
            if element.kind != ElementKind::Hook {
                continue;
            }
            let implemented = cx.resolved.impl_ends.iter().flatten().any(|(by, hook)| {
                hook.index() == i
                    && matches!(cx.element(*by).kind, ElementKind::Api | ElementKind::Module)
            });
            if !implemented {
                out.push(
                    Diagnostic::new(
                        "I202",
                        Severity::Info,
                        format!("hook `{}` has no implementation in the library", element.name.text),
                        element.span,
                        "every consumer must provide one",
                    )
                    .with_help(format!(
                        "add a default (`<module> impl {}`), or `allow I202` if consumers are expected to bring their own",
                        element.name.text
                    )),
                );
            }
        }
    }
}
