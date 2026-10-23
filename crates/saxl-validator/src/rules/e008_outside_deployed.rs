//! E008: an element outside the system or library is deployed or published.

use saxl_core::{Diagnostic, ElementKind, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E008"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "an element outside the system or library is deployed or published"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let elements = cx.model.elements.iter();
        if cx.is_library() {
            for (element, publications) in elements.zip(&cx.resolved.publications) {
                if matches!(element.kind, ElementKind::Api | ElementKind::Module) {
                    continue;
                }
                for p in publications {
                    out.push(
                        Diagnostic::error(
                            "E008",
                            format!(
                                "`{}` is {} {} and can't be published",
                                element.name.text,
                                element.kind.article(),
                                element.kind.keyword()
                            ),
                            p.span,
                            format!("{}s are not part of the library", element.kind.keyword()),
                        )
                        .with_help("only apis and modules are published"),
                    );
                }
            }
            return;
        }
        for (element, deployments) in elements.zip(&cx.resolved.deployments) {
            if element.kind.is_internal() {
                continue;
            }
            for d in deployments {
                out.push(
                    Diagnostic::error(
                        "E008",
                        format!(
                            "`{}` is {} {} and can't be deployed",
                            element.name.text,
                            element.kind.article(),
                            element.kind.keyword()
                        ),
                        d.span,
                        format!("{}s run outside the system", element.kind.keyword()),
                    )
                    .with_help("only services, stores, and queues are deployed"),
                );
            }
        }
    }
}
