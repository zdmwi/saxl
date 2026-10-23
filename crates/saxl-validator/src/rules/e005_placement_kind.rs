//! E005: a data placement names the wrong kind of element.

use saxl_core::{Diagnostic, ElementKind, NameRef, Placement, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E005"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "a data placement names the wrong kind of element"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for (data, broken) in cx.model.data.iter().zip(&cx.broken.data) {
            if *broken {
                continue;
            }
            let mut want = |r: &NameRef, kind: ElementKind, role: &str| {
                let Some(id) = cx.resolved.element(&r.text) else {
                    return;
                };
                let actual = cx.element(id).kind;
                if actual != kind {
                    out.push(
                        Diagnostic::error(
                            "E005",
                            format!("{role} must be a {}", kind.keyword()),
                            r.span,
                            format!("`{}` is a {}", r.text, actual.keyword()),
                        )
                        .with_help(format!(
                            "name a {} here, or declare `{}` as one",
                            kind.keyword(),
                            r.text
                        )),
                    );
                }
            };
            match &data.placement {
                Placement::In { store, cached } => {
                    want(store, ElementKind::Store, "the `in` target");
                    for c in cached {
                        want(c, ElementKind::Store, "a `cached` target");
                    }
                }
                Placement::Via { queue, store } => {
                    want(queue, ElementKind::Queue, "the `via` source");
                    want(store, ElementKind::Store, "the `via` destination");
                }
            }
        }
    }
}
