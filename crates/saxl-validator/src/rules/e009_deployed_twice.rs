//! E009: an element is deployed twice in one region, or published twice.

use indexmap::IndexMap;
use saxl_core::{Diagnostic, ElementKind, Severity, Span};

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
        "an element is deployed twice in one region, or published twice"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        if cx.is_library() {
            published_twice(cx, out);
            return;
        }
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

/// A name listed twice in one package, or a module in two packages. An api
/// may appear in several packages: a package can re-export it.
fn published_twice(cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
    for (element, publications) in cx.model.elements.iter().zip(&cx.resolved.publications) {
        for (i, p) in publications.iter().enumerate() {
            let earlier = &publications[..i];
            let same_package = earlier
                .iter()
                .find(|q| (q.registry, q.package) == (p.registry, p.package));
            let package = &cx.model.registries[p.registry].packages[p.package]
                .name
                .text;
            if let Some(first) = same_package {
                out.push(
                    Diagnostic::error(
                        "E009",
                        format!(
                            "`{}` is listed twice in package `{package}`",
                            element.name.text
                        ),
                        p.span,
                        "listed again here",
                    )
                    .with_secondary(first.span, "first listed here")
                    .with_help("list each element once per package"),
                );
            } else if let (ElementKind::Module, Some(first)) = (element.kind, earlier.first()) {
                out.push(
                    Diagnostic::error(
                        "E009",
                        format!("module `{}` is published in two packages", element.name.text),
                        p.span,
                        format!("published again in `{package}`"),
                    )
                    .with_secondary(first.span, "first published here")
                    .with_help("a module belongs to one package; only an api can be re-exported by another"),
                );
            }
        }
    }
}
