//! W103: a source-of-truth store has no replica.

use std::collections::BTreeSet;

use saxl_core::{Diagnostic, ElementKind, Placement, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W103"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "a source-of-truth store has no `primary+replica`"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let mut stores = BTreeSet::new();
        for (data, broken) in cx.model.data.iter().zip(&cx.broken.data) {
            if *broken {
                continue;
            }
            let store = match &data.placement {
                Placement::In { store, .. } | Placement::Via { store, .. } => store,
            };
            if let Some(id) = cx.resolved.element(&store.text) {
                stores.insert(id);
            }
        }
        for id in stores {
            let element = cx.element(id);
            let deployments = &cx.resolved.deployments[id.index()];
            if element.kind != ElementKind::Store || deployments.iter().any(|d| d.primary_replica) {
                continue;
            }
            let Some(first) = deployments.first() else {
                continue;
            };
            out.push(
                Diagnostic::new(
                    "W103",
                    Severity::Warning,
                    format!(
                        "store `{}` is a source of truth but has no replica",
                        element.name.text
                    ),
                    first.span,
                    "no `primary+replica`",
                )
                .with_help(
                    "add `primary+replica` to its workload so losing one node doesn't lose data",
                ),
            );
        }
    }
}
