//! E009: an element is deployed twice in one region.

use indexmap::IndexMap;
use saxl_core::{Diagnostic, Severity, Span};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "E009"
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn summary(&self) -> &'static str {
        "an element is deployed twice in one region"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        for region in &cx.model.regions {
            let mut seen: IndexMap<usize, Span> = IndexMap::new();
            let names = region
                .groups
                .iter()
                .flat_map(|g| &g.workloads)
                .flat_map(|w| &w.names);
            for name in names {
                let Some(id) = cx.resolved.element(&name.text) else {
                    continue;
                };
                if let Some(first) = seen.get(&id.index()) {
                    out.push(
                        Diagnostic::error(
                            "E009",
                            format!("`{}` is deployed twice in region `{}`", name.text, region.name.text),
                            name.span,
                            "deployed again here",
                        )
                        .with_secondary(*first, "first deployed here")
                        .with_help("deploy it once per region and use a replica count such as `x3` to scale it"),
                    );
                } else {
                    seen.insert(id.index(), name.span);
                }
            }
        }
    }
}
