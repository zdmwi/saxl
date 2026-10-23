//! W110: a library's api or module is in no package.

use saxl_core::{Diagnostic, ElementKind, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W110"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "a library's api or module is in no package"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if !cx.is_library() {
            return;
        }
        for (element, publications) in cx.model.elements.iter().zip(&cx.resolved.publications) {
            if matches!(element.kind, ElementKind::Api | ElementKind::Module)
                && publications.is_empty()
            {
                out.push(
                    Diagnostic::new(
                        "W110",
                        Severity::Warning,
                        format!("`{}` is not in any package", element.name.text),
                        element.span,
                        "never shipped",
                    )
                    .with_help("add it to a `crate` or `package` in a `publish` block"),
                );
            }
        }
    }
}
