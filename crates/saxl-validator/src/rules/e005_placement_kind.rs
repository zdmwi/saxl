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
        let library = cx.is_library();
        for (data, broken) in cx.model.data.iter().zip(&cx.broken.data) {
            if *broken {
                continue;
            }
            // `wanted` names the accepted kinds, for messages: "a store".
            let mut want = |r: &NameRef, kinds: &[ElementKind], wanted: &str, role: &str| {
                let Some(id) = cx.resolved.element(&r.text) else {
                    return;
                };
                let actual = cx.element(id).kind;
                if !kinds.contains(&actual) {
                    out.push(
                        Diagnostic::error(
                            "E005",
                            format!("{role} must be {wanted}"),
                            r.span,
                            format!("`{}` is {} {}", r.text, actual.article(), actual.keyword()),
                        )
                        .with_help(format!(
                            "name {wanted} here, or declare `{}` as one",
                            r.text
                        )),
                    );
                }
            };
            let code = [ElementKind::Api, ElementKind::Module];
            match &data.placement {
                Placement::In { store, cached } if library => {
                    want(store, &code, "an api or module", "the `in` target");
                    for c in cached {
                        out.push(
                            Diagnostic::error(
                                "E005",
                                "a library type can't be cached",
                                c.span,
                                "`cached` only applies to stores",
                            )
                            .with_help(
                                "remove `cached …`; a type lives in the module that owns it",
                            ),
                        );
                    }
                }
                Placement::Via { queue, .. } if library => out.push(
                    Diagnostic::error(
                        "E005",
                        "a library type can't flow through a queue",
                        queue.span,
                        "`via` only applies to queues",
                    )
                    .with_help("write `data <Name> in <module>` instead"),
                ),
                Placement::In { store, cached } => {
                    want(store, &[ElementKind::Store], "a store", "the `in` target");
                    for c in cached {
                        want(c, &[ElementKind::Store], "a store", "a `cached` target");
                    }
                }
                Placement::Via { queue, store } => {
                    want(queue, &[ElementKind::Queue], "a queue", "the `via` source");
                    want(
                        store,
                        &[ElementKind::Store],
                        "a store",
                        "the `via` destination",
                    );
                }
            }
        }
    }
}
