//! W107: synchronous calls form a cycle.

use petgraph::algo::tarjan_scc;
use saxl_core::{Diagnostic, Severity, Style};

use super::Context;

pub(crate) struct Rule;

impl super::Rule for Rule {
    fn code(&self) -> &'static str {
        "W107"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn summary(&self) -> &'static str {
        "synchronous calls form a cycle"
    }

    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>) {
        let sync = cx
            .resolved
            .graph
            .filter_map(|_, n| Some(*n), |_, s| (*s == Style::Sync).then_some(()));
        let mut cycles: Vec<Vec<usize>> = Vec::new();
        for scc in tarjan_scc(&sync) {
            let cyclic = scc.len() > 1 || scc.first().is_some_and(|n| sync.contains_edge(*n, *n));
            if !cyclic {
                continue;
            }
            let members: Vec<_> = scc.iter().map(|n| sync[*n]).collect();
            let relations: Vec<usize> = cx
                .model
                .relations
                .iter()
                .zip(&cx.resolved.relation_ends)
                .enumerate()
                .filter(|(_, (r, ends))| {
                    r.style == Style::Sync
                        && ends.is_some_and(|(f, t)| members.contains(&f) && members.contains(&t))
                })
                .map(|(i, _)| i)
                .collect();
            if !relations.is_empty() {
                cycles.push(relations);
            }
        }
        cycles.sort();
        for relations in cycles {
            let rels = &cx.model.relations;
            let first = &rels[relations[0]];
            let mut d = Diagnostic::new(
                "W107",
                Severity::Warning,
                "synchronous calls form a cycle",
                first.span,
                "this call starts the cycle",
            )
            .with_help("a synchronous cycle can deadlock or recurse without bound; make one call async (`~>`) or remove it");
            for &i in &relations[1..] {
                d = d.with_secondary(rels[i].span, "part of the cycle");
            }
            out.push(d);
        }
    }
}
