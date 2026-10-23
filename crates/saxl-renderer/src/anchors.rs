//! Source spans that name each element, infrastructure node, data entity, and
//! the system, so the viewer can map a diagnostic's primary label to the nodes
//! it should mark.

use indexmap::IndexMap;
use saxl_core::{Model, NameRef, Placement, Span, Step};

use crate::scene::{Anchor, NodeKind, Scene};

pub(crate) struct SpanIndex {
    /// Element and infrastructure names: declaration and every reference.
    names: IndexMap<String, Vec<Span>>,
    /// Data entity declarations.
    data: IndexMap<String, Span>,
    system: Option<Span>,
}

impl SpanIndex {
    pub fn new(model: &Model) -> Self {
        let mut names: IndexMap<String, Vec<Span>> = IndexMap::new();
        let mut add = |text: &str, span: Span| names.entry(text.to_owned()).or_default().push(span);
        for e in &model.elements {
            add(&e.name.text, e.span);
        }
        for region in &model.regions {
            for n in &region.infra {
                add(&n.name.text, n.span);
            }
        }
        let mut refs: Vec<&NameRef> = Vec::new();
        for r in &model.relations {
            refs.extend([&r.from, &r.to]);
        }
        for d in &model.data {
            match &d.placement {
                Placement::In { store, cached } => {
                    refs.push(store);
                    refs.extend(cached);
                }
                Placement::Via { queue, store } => refs.extend([queue, store]),
            }
        }
        for region in &model.regions {
            for g in &region.groups {
                for w in &g.workloads {
                    refs.extend(&w.names);
                }
            }
        }
        fn hops<'a>(steps: &'a [Step], out: &mut Vec<&'a NameRef>) {
            for step in steps {
                match step {
                    Step::Message { hops: h, .. } => out.extend(h),
                    Step::Parallel { steps, .. } => hops(steps, out),
                }
            }
        }
        for s in &model.scenarios {
            hops(&s.steps, &mut refs);
        }
        for r in refs {
            add(&r.text, r.span);
        }
        Self {
            names,
            data: model
                .data
                .iter()
                .map(|d| (d.name.text.clone(), d.span))
                .collect(),
            system: model.roots.first().map(|s| s.span),
        }
    }

    fn spans_for(&self, id: &str, kind: NodeKind) -> Vec<Span> {
        if kind == NodeKind::System {
            return self.system.into_iter().collect();
        }
        if let Some(entity) = id.strip_prefix("data:") {
            return self.data.get(entity).copied().into_iter().collect();
        }
        // Deployment nodes are `region/name`.
        let name = id.rsplit('/').next().unwrap_or(id);
        self.names.get(name).cloned().unwrap_or_default()
    }

    /// Anchors for every node and tower box in `scene`.
    pub fn anchors(&self, scene: &Scene) -> Vec<Anchor> {
        let mut ids: Vec<(&str, NodeKind)> = scene
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.kind))
            .collect();
        if let Some(t) = &scene.tower {
            for b in t.containers.iter().chain(&t.outside).chain(&t.infra) {
                ids.push((&b.id, b.kind));
            }
        }
        let mut out = Vec::new();
        for (id, kind) in ids {
            for span in self.spans_for(id, kind) {
                out.push(Anchor {
                    node: id.to_owned(),
                    span,
                });
            }
        }
        out
    }
}
