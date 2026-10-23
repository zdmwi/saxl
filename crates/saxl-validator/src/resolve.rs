//! Name resolution: builds the symbol table, resolves every reference, and
//! records which statements must be skipped by later rules.

use std::collections::BTreeSet;

use indexmap::IndexMap;
use indexmap::map::Entry;
use petgraph::graph::{DiGraph, NodeIndex};
use saxl_core::{Diagnostic, Model, NameRef, Placement, Span, Step, Style};

/// Index into [`Model::elements`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(pub u32);

impl ElementId {
    /// The index as `usize`.
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// The element's node in [`Resolved::graph`].
    pub fn node(self) -> NodeIndex {
        NodeIndex::new(self.index())
    }
}

/// Index into [`Resolved::infra`], the flattened list of infrastructure nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InfraId(pub u32);

/// What a name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// An element.
    Element(ElementId),
    /// An infrastructure node.
    Infra(InfraId),
}

/// Where one element runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployedElement {
    /// Index into [`Model::regions`].
    pub region: usize,
    /// Index into the region's groups.
    pub group: usize,
    /// Index into the group's workloads.
    pub workload: usize,
    /// Replica count.
    pub replicas: u32,
    /// Whether the workload autoscales.
    pub autoscale: bool,
    /// Whether the workload has `primary+replica`.
    pub primary_replica: bool,
    /// Where the element is named in the workload line.
    pub span: Span,
}

/// The result of name resolution.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Every declared name. The first declaration wins on duplicates.
    pub names: IndexMap<String, Target>,
    /// Infrastructure nodes as `(region index, index within region)`.
    pub infra: Vec<(usize, usize)>,
    /// Parallel to [`Model::relations`]. `None` only for relations with a broken
    /// reference, so always `Some` in a validated model.
    pub relation_ends: Vec<Option<(ElementId, ElementId)>>,
    /// One node per element, in declaration order; one edge per resolved relation.
    pub graph: DiGraph<ElementId, Style>,
    /// Per element: where it runs.
    pub deployments: Vec<Vec<DeployedElement>>,
}

impl Resolved {
    /// What `name` refers to.
    pub fn lookup(&self, name: &str) -> Option<Target> {
        self.names.get(name).copied()
    }

    /// The element `name` refers to, if it is an element.
    pub fn element(&self, name: &str) -> Option<ElementId> {
        match self.lookup(name) {
            Some(Target::Element(id)) => Some(id),
            _ => None,
        }
    }
}

/// Statements that later rules skip because one of their references failed to
/// resolve or named the wrong sort of thing.
#[derive(Debug, Clone, Default)]
pub struct BrokenRefs {
    /// Parallel to [`Model::relations`].
    pub relations: Vec<bool>,
    /// Parallel to [`Model::data`].
    pub data: Vec<bool>,
    /// Parallel to [`Model::scenarios`].
    pub scenarios: Vec<bool>,
    /// `(region, group, workload)` triples.
    pub workloads: BTreeSet<(usize, usize, usize)>,
    /// E001, E002, and E011 diagnostics found while resolving.
    pub(crate) diagnostics: Vec<Diagnostic>,
}

pub(crate) fn resolve(model: &Model) -> (Resolved, BrokenRefs) {
    let mut broken = BrokenRefs {
        relations: vec![false; model.relations.len()],
        data: vec![false; model.data.len()],
        scenarios: vec![false; model.scenarios.len()],
        ..BrokenRefs::default()
    };

    // Symbol table, in declaration order.
    let mut decls: Vec<(&str, Span, Target)> = Vec::new();
    for (i, e) in model.elements.iter().enumerate() {
        decls.push((
            &e.name.text,
            e.name.span,
            Target::Element(ElementId(to_u32(i))),
        ));
    }
    let mut infra = Vec::new();
    for (r, region) in model.regions.iter().enumerate() {
        for (n, node) in region.infra.iter().enumerate() {
            decls.push((
                &node.name.text,
                node.name.span,
                Target::Infra(InfraId(to_u32(infra.len()))),
            ));
            infra.push((r, n));
        }
    }
    decls.sort_by_key(|(_, span, _)| span.start);
    let mut names = IndexMap::new();
    let mut first_spans: IndexMap<&str, Span> = IndexMap::new();
    for (name, span, target) in decls {
        match first_spans.entry(name) {
            Entry::Occupied(first) => broken.diagnostics.push(
                Diagnostic::error(
                    "E001",
                    format!("`{name}` is declared twice"),
                    span,
                    "declared again here",
                )
                .with_secondary(*first.get(), "first declared here")
                .with_help(
                    "rename one of them; elements and infrastructure nodes share one namespace",
                ),
            ),
            Entry::Vacant(slot) => {
                slot.insert(span);
                names.insert(name.to_owned(), target);
            }
        }
    }

    let mut cx = Resolver {
        names: &names,
        diagnostics: &mut broken.diagnostics,
    };

    let mut relation_ends = Vec::with_capacity(model.relations.len());
    for (i, rel) in model.relations.iter().enumerate() {
        let from = cx.element(&rel.from, "a relation");
        let to = cx.element(&rel.to, "a relation");
        match (from, to) {
            (Some(f), Some(t)) => relation_ends.push(Some((f, t))),
            _ => {
                broken.relations[i] = true;
                relation_ends.push(None);
            }
        }
    }

    for (i, data) in model.data.iter().enumerate() {
        let refs: Vec<&NameRef> = match &data.placement {
            Placement::In { store, cached } => std::iter::once(store).chain(cached).collect(),
            Placement::Via { queue, store } => vec![queue, store],
        };
        let mut ok = true;
        for r in refs {
            ok &= cx.element(r, "a data placement").is_some();
        }
        broken.data[i] = !ok;
    }

    let mut deployments = vec![Vec::new(); model.elements.len()];
    for (r, region) in model.regions.iter().enumerate() {
        for (g, group) in region.groups.iter().enumerate() {
            for (w, workload) in group.workloads.iter().enumerate() {
                for name in &workload.names {
                    match cx.element(name, "a deployment workload") {
                        Some(id) => deployments[id.index()].push(DeployedElement {
                            region: r,
                            group: g,
                            workload: w,
                            replicas: workload.replicas(),
                            autoscale: workload.autoscale(),
                            primary_replica: workload.primary_replica(),
                            span: name.span,
                        }),
                        None => {
                            broken.workloads.insert((r, g, w));
                        }
                    }
                }
            }
        }
    }

    for (i, scenario) in model.scenarios.iter().enumerate() {
        broken.scenarios[i] = !cx.steps(&scenario.steps);
    }

    let mut graph = DiGraph::with_capacity(model.elements.len(), model.relations.len());
    for i in 0..model.elements.len() {
        graph.add_node(ElementId(to_u32(i)));
    }
    for (rel, ends) in model.relations.iter().zip(&relation_ends) {
        if let Some((f, t)) = ends {
            graph.add_edge(f.node(), t.node(), rel.style);
        }
    }

    let resolved = Resolved {
        names,
        infra,
        relation_ends,
        graph,
        deployments,
    };
    (resolved, broken)
}

struct Resolver<'a> {
    names: &'a IndexMap<String, Target>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl Resolver<'_> {
    fn lookup(&mut self, r: &NameRef) -> Option<Target> {
        let target = self.names.get(&r.text).copied();
        if target.is_none() {
            let mut d = Diagnostic::error(
                "E002",
                format!("`{}` is not declared", r.text),
                r.span,
                "not declared",
            );
            if let Some(near) = suggest(&r.text, self.names.keys()) {
                d = d.with_help(format!("did you mean `{near}`?"));
            }
            self.diagnostics.push(d);
        }
        target
    }

    /// Resolves a reference that must name an element.
    fn element(&mut self, r: &NameRef, place: &str) -> Option<ElementId> {
        match self.lookup(r)? {
            Target::Element(id) => Some(id),
            Target::Infra(_) => {
                self.diagnostics.push(infra_misuse(r, place));
                None
            }
        }
    }

    /// Resolves every hop in `steps`. Returns false if any failed.
    fn steps(&mut self, steps: &[Step]) -> bool {
        let mut ok = true;
        for step in steps {
            match step {
                Step::Message { hops, .. } => {
                    for (i, hop) in hops.iter().enumerate() {
                        match self.lookup(hop) {
                            None => ok = false,
                            Some(Target::Infra(_)) if i == 0 || i + 1 == hops.len() => {
                                self.diagnostics
                                    .push(infra_misuse(hop, "the start or end of a scenario step"));
                                ok = false;
                            }
                            Some(_) => {}
                        }
                    }
                }
                Step::Parallel { steps, .. } => ok &= self.steps(steps),
            }
        }
        ok
    }
}

fn infra_misuse(r: &NameRef, place: &str) -> Diagnostic {
    Diagnostic::error(
        "E011",
        format!("infrastructure node `{}` can't be used in {place}", r.text),
        r.span,
        "infrastructure node",
    )
    .with_help("infrastructure nodes are only allowed between other hops of a scenario step")
}

/// The closest declared name within edit distance 2.
fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a String>) -> Option<&'a str> {
    candidates
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, c)| *d <= 2 && *d < c.chars().count().max(name.chars().count()))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.as_str())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        assert_eq!(edit_distance("Redis", "Redis"), 0);
        assert_eq!(edit_distance("Rediss", "Redis"), 1);
        assert_eq!(edit_distance("Postgress", "Postgres"), 1);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn suggestions() {
        let names = ["Postgres".to_owned(), "Redis".to_owned()];
        assert_eq!(suggest("Postgre", names.iter()), Some("Postgres"));
        assert_eq!(suggest("Kafka", names.iter()), None);
    }
}
