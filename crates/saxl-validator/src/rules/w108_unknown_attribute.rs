//! W108: unknown deployment or publish attribute.

use saxl_core::{Attribute, Diagnostic, Severity};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W108"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "unknown deployment or publish attribute"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let deployed = cx
            .model
            .regions
            .iter()
            .flat_map(|r| &r.groups)
            .flat_map(|g| &g.workloads);
        let published = cx
            .model
            .registries
            .iter()
            .flat_map(|r| &r.packages)
            .flat_map(|p| &p.workloads);
        let blocks = [
            (
                deployed.collect::<Vec<_>>(),
                "deployment",
                "known attributes are `xN` (replicas), `autoscale`, and `primary+replica`",
            ),
            (
                published.collect(),
                "publish",
                "the known attribute is `feature <name>`",
            ),
        ];
        for (workloads, block, help) in blocks {
            for attr in workloads.iter().flat_map(|w| &w.attributes) {
                if let Attribute::Unknown { text, span } = attr {
                    out.push(
                        Diagnostic::new(
                            "W108",
                            Severity::Warning,
                            format!("unknown {block} attribute `{text}`"),
                            *span,
                            "not a known attribute",
                        )
                        .with_help(help),
                    );
                }
            }
        }
    }
}
