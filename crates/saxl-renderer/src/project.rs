//! Projections: pure functions from a validated model to a view-specific graph
//! with no geometry. The layouts in `layout/` add the geometry.

use saxl_core::{
    Attribute, ElementKind, GroupKind as IrGroupKind, Model, NameRef, PackageKind, Placement,
    RootKind, Span, Step, StepArrow,
};
use saxl_validator::{Target, ValidatedModel};

use crate::RenderError;
use crate::scene::{EdgeStyle, GroupKind, NodeKind, Timeline, TimelineStep};

/// A node before layout.
#[derive(Debug, Clone)]
pub(crate) struct PNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub sublabel: Option<String>,
    pub source: Option<Span>,
    pub badges: Vec<String>,
    pub fields: Vec<String>,
}

impl PNode {
    fn new(id: impl Into<String>, kind: NodeKind, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            label: label.into(),
            sublabel: None,
            source: None,
            badges: Vec::new(),
            fields: Vec::new(),
        }
    }
}

/// An edge between node indices.
#[derive(Debug, Clone)]
pub(crate) struct PEdge {
    pub from: usize,
    pub to: usize,
    pub style: EdgeStyle,
    pub label: Option<String>,
}

pub(crate) fn element_node(model: &Model, index: usize) -> PNode {
    let e = &model.elements[index];
    PNode {
        sublabel: e.description.clone(),
        source: Some(e.span),
        ..PNode::new(&e.name.text, e.kind.into(), &e.name.text)
    }
}

/// Containers (Modules, for a library): every element, every relation, and
/// every `impl`.
pub(crate) struct Containers {
    pub nodes: Vec<PNode>,
    pub edges: Vec<PEdge>,
}

pub(crate) fn containers(vm: &ValidatedModel) -> Containers {
    let model = vm.model();
    let nodes = (0..model.elements.len())
        .map(|i| {
            let mut node = element_node(model, i);
            node.badges = feature_badges(vm, i);
            node
        })
        .collect();
    let mut edges: Vec<PEdge> = model
        .relations
        .iter()
        .zip(&vm.resolved().relation_ends)
        .filter_map(|(rel, ends)| {
            let (from, to) = (*ends)?;
            Some(PEdge {
                from: from.index(),
                to: to.index(),
                style: rel.style.into(),
                label: rel.description.clone(),
            })
        })
        .collect();
    edges.extend(
        vm.resolved()
            .impl_ends
            .iter()
            .flatten()
            .map(|(by, hook)| PEdge {
                from: by.index(),
                to: hook.index(),
                style: EdgeStyle::Impl,
                label: None,
            }),
    );
    Containers { nodes, edges }
}

/// `feature f` badges for the features an element is gated on, in order of
/// first appearance.
fn feature_badges(vm: &ValidatedModel, index: usize) -> Vec<String> {
    let mut badges: Vec<String> = Vec::new();
    for p in &vm.resolved().publications[index] {
        if let Some(f) = &p.feature {
            let badge = format!("feature {f}");
            if !badges.contains(&badge) {
                badges.push(badge);
            }
        }
    }
    badges
}

/// Context: the system or library as one box; actors and consumers left;
/// externals, packages, and hooks right.
pub(crate) struct Context {
    pub system: PNode,
    pub actors: Vec<PNode>,
    pub externals: Vec<PNode>,
    /// `(outside node id, outside → system?, style)`, one per outside element
    /// that relates to anything inside.
    pub links: Vec<(String, bool, EdgeStyle)>,
}

pub(crate) fn context(vm: &ValidatedModel) -> Context {
    let model = vm.model();
    let mut system = PNode::new("system", NodeKind::System, "");
    if let Some(s) = model.roots.first() {
        if s.kind == RootKind::Library {
            system.kind = NodeKind::Library;
        }
        system.id.clone_from(&s.name.text);
        system.label.clone_from(&s.name.text);
        system.sublabel.clone_from(&s.description);
        system.source = Some(s.span);
    }
    let (mut actors, mut externals, mut links) = (Vec::new(), Vec::new(), Vec::new());
    for (i, e) in model.elements.iter().enumerate() {
        let list = match e.kind {
            ElementKind::Actor | ElementKind::Consumer => &mut actors,
            ElementKind::External | ElementKind::Package | ElementKind::Hook => &mut externals,
            _ => continue,
        };
        list.push(element_node(model, i));
        let mut inbound = false;
        let mut outbound = false;
        let mut sync = false;
        for (rel, ends) in model.relations.iter().zip(&vm.resolved().relation_ends) {
            let Some((from, to)) = ends else { continue };
            let other = if from.index() == i {
                outbound = true;
                *to
            } else if to.index() == i {
                inbound = true;
                *from
            } else {
                continue;
            };
            if !model.elements[other.index()].kind.is_internal() {
                continue;
            }
            sync |= rel.style == saxl_core::Style::Sync;
        }
        if inbound || outbound {
            let style = if sync {
                EdgeStyle::Sync
            } else {
                EdgeStyle::Async
            };
            links.push((e.name.text.clone(), outbound, style));
        }
    }
    Context {
        system,
        actors,
        externals,
        links,
    }
}

/// Deployment: regions of groups of workload nodes. Packages, for a library,
/// use the same shape: registries of packages of published elements.
pub(crate) struct DeployRegion {
    pub id: String,
    pub kind: GroupKind,
    pub label: String,
    pub infra: Vec<PNode>,
    pub groups: Vec<DeployGroup>,
}

pub(crate) struct DeployGroup {
    pub id: String,
    pub kind: GroupKind,
    pub label: String,
    pub nodes: Vec<PNode>,
}

pub(crate) fn deployment(vm: &ValidatedModel) -> Vec<DeployRegion> {
    let model = vm.model();
    model
        .regions
        .iter()
        .map(|region| {
            let r = &region.name.text;
            let infra = region
                .infra
                .iter()
                .map(|n| PNode {
                    source: Some(n.span),
                    sublabel: Some("cdn".to_owned()),
                    ..PNode::new(
                        format!("{r}/{}", n.name.text),
                        NodeKind::Infra,
                        &n.name.text,
                    )
                })
                .collect();
            let groups = region
                .groups
                .iter()
                .enumerate()
                .map(|(g, group)| {
                    let (kind, label) = match group.kind {
                        IrGroupKind::Cluster => (
                            GroupKind::Cluster,
                            group
                                .name
                                .as_ref()
                                .map_or_else(|| "cluster".to_owned(), |n| n.text.clone()),
                        ),
                        IrGroupKind::Managed => (GroupKind::Managed, "managed".to_owned()),
                    };
                    let mut nodes = Vec::new();
                    for w in &group.workloads {
                        let badges: Vec<String> = w
                            .attributes
                            .iter()
                            .map(|a| match a {
                                Attribute::Replicas { count, .. } => format!("×{count}"),
                                Attribute::Autoscale { .. } => "autoscale".to_owned(),
                                Attribute::PrimaryReplica { .. } => "primary+replica".to_owned(),
                                Attribute::Feature { name, .. } => format!("feature {}", name.text),
                                Attribute::Unknown { text, .. } => text.clone(),
                            })
                            .collect();
                        for name in &w.names {
                            let Some(id) = vm.resolved().element(&name.text) else {
                                continue;
                            };
                            let mut node = element_node(model, id.index());
                            node.id = format!("{r}/{}", name.text);
                            node.badges.clone_from(&badges);
                            nodes.push(node);
                        }
                    }
                    DeployGroup {
                        id: format!("{r}/{g}"),
                        kind,
                        label,
                        nodes,
                    }
                })
                .collect();
            DeployRegion {
                id: r.clone(),
                kind: GroupKind::Region,
                label: r.clone(),
                infra,
                groups,
            }
        })
        .collect()
}

pub(crate) fn packages(vm: &ValidatedModel) -> Vec<DeployRegion> {
    let model = vm.model();
    model
        .registries
        .iter()
        .map(|registry| {
            let r = &registry.name.text;
            let groups = registry
                .packages
                .iter()
                .enumerate()
                .map(|(p, package)| {
                    let mut nodes = Vec::new();
                    for w in &package.workloads {
                        let badges: Vec<String> = w
                            .feature()
                            .map(|f| format!("feature {}", f.text))
                            .into_iter()
                            .collect();
                        for name in &w.names {
                            let Some(id) = vm.resolved().element(&name.text) else {
                                continue;
                            };
                            let mut node = element_node(model, id.index());
                            node.id = format!("{r}/{}/{}", package.name.text, name.text);
                            node.badges.clone_from(&badges);
                            nodes.push(node);
                        }
                    }
                    let keyword = match package.kind {
                        PackageKind::Crate => "crate",
                        PackageKind::Package => "package",
                    };
                    let mut label = format!("{keyword} {}", package.name.text);
                    if let Some(t) = &package.target {
                        label.push_str(&format!(" · {}", t.text));
                    }
                    DeployGroup {
                        id: format!("{r}/{p}"),
                        kind: GroupKind::Package,
                        label,
                        nodes,
                    }
                })
                .collect();
            DeployRegion {
                id: r.clone(),
                kind: GroupKind::Registry,
                label: r.clone(),
                infra: Vec::new(),
                groups,
            }
        })
        .collect()
}

/// Runtime: participants and one row per element-to-element hop.
pub(crate) struct Runtime {
    pub title: String,
    pub participants: Vec<PNode>,
    pub rows: Vec<Row>,
    /// `(first row, last row, depth)` for each non-empty `parallel` block.
    pub parallels: Vec<(usize, usize, u32)>,
    pub timeline: Timeline,
}

/// One row: the raw hops (including infrastructure) between two elements.
pub(crate) struct Row {
    /// `(from participant, to participant, style)` per raw hop.
    pub hops: Vec<(usize, usize, EdgeStyle)>,
    pub label: Option<String>,
}

pub(crate) fn runtime(vm: &ValidatedModel, name: &str) -> Result<Runtime, RenderError> {
    let model = vm.model();
    let scenario = model
        .scenarios
        .iter()
        .find(|s| s.name.text == name)
        .ok_or_else(|| RenderError::UnknownScenario(name.to_owned()))?;
    let mut b = RuntimeBuilder {
        vm,
        participants: Vec::new(),
        rows: Vec::new(),
        parallels: Vec::new(),
        steps: Vec::new(),
    };
    b.steps(&scenario.steps, 0, 0);
    Ok(Runtime {
        title: format!("Runtime: {name}"),
        participants: b.participants,
        rows: b.rows,
        parallels: b.parallels,
        timeline: Timeline {
            scenario: name.to_owned(),
            steps: b.steps,
        },
    })
}

struct RuntimeBuilder<'a> {
    vm: &'a ValidatedModel,
    participants: Vec<PNode>,
    rows: Vec<Row>,
    parallels: Vec<(usize, usize, u32)>,
    steps: Vec<TimelineStep>,
}

impl RuntimeBuilder<'_> {
    fn participant(&mut self, r: &NameRef) -> usize {
        if let Some(i) = self.participants.iter().position(|p| p.id == r.text) {
            return i;
        }
        let model = self.vm.model();
        let node = match self.vm.resolved().lookup(&r.text) {
            Some(Target::Element(id)) => element_node(model, id.index()),
            Some(Target::Infra(id)) => {
                let (region, n) = self.vm.resolved().infra[id.0 as usize];
                let infra = &model.regions[region].infra[n];
                PNode {
                    source: Some(infra.span),
                    sublabel: Some("cdn".to_owned()),
                    ..PNode::new(&r.text, NodeKind::Infra, &r.text)
                }
            }
            None => PNode::new(&r.text, NodeKind::Service, &r.text),
        };
        self.participants.push(node);
        self.participants.len() - 1
    }

    fn is_infra(&self, r: &NameRef) -> bool {
        matches!(self.vm.resolved().lookup(&r.text), Some(Target::Infra(_)))
    }

    /// Lays out `steps` starting at playback `slot`; returns the next free slot.
    fn steps(&mut self, steps: &[Step], mut slot: u32, depth: u32) -> u32 {
        for step in steps {
            match step {
                Step::Message {
                    hops,
                    arrows,
                    label,
                    ..
                } => slot = self.message(hops, arrows, label.as_ref(), slot),
                Step::Parallel { steps, .. } => {
                    let first_row = self.rows.len();
                    let start = slot;
                    let mut end = slot;
                    for branch in steps {
                        end = end.max(self.steps(std::slice::from_ref(branch), start, depth + 1));
                    }
                    if self.rows.len() > first_row {
                        self.parallels.push((first_row, self.rows.len() - 1, depth));
                    }
                    slot = end;
                }
            }
        }
        slot
    }

    fn message(
        &mut self,
        hops: &[NameRef],
        arrows: &[StepArrow],
        label: Option<&String>,
        mut slot: u32,
    ) -> u32 {
        let indices: Vec<usize> = hops.iter().map(|h| self.participant(h)).collect();
        // Split the hop list at elements: each segment is one row.
        let mut start = 0;
        let mut segments = Vec::new();
        for (i, hop) in hops.iter().enumerate().skip(1) {
            if !self.is_infra(hop) {
                segments.push((start, i));
                start = i;
            }
        }
        let last = segments.len().saturating_sub(1);
        for (n, (a, b)) in segments.into_iter().enumerate() {
            let row_label = if n == last { label.cloned() } else { None };
            let arrow = arrows.get(a).copied().unwrap_or(StepArrow::Call);
            self.steps.push(TimelineStep {
                index: slot,
                from: hops[a].text.clone(),
                to: hops[b].text.clone(),
                via: hops[a + 1..b].iter().map(|h| h.text.clone()).collect(),
                arrow: arrow.into(),
                label: row_label.clone(),
                row: Some(u32::try_from(self.rows.len()).unwrap_or(u32::MAX)),
            });
            self.rows.push(Row {
                hops: (a..b)
                    .map(|i| {
                        (
                            indices[i],
                            indices[i + 1],
                            arrows.get(i).copied().unwrap_or(arrow).into(),
                        )
                    })
                    .collect(),
                label: row_label,
            });
            slot += 1;
        }
        slot
    }
}

/// Data: store nodes above entity cards.
pub(crate) struct Data {
    pub stores: Vec<PNode>,
    pub cards: Vec<PNode>,
    /// `(card, store, style, meaning)`. The meaning is not drawn: the card's
    /// sublabel already says where its data lives.
    pub links: Vec<(usize, usize, EdgeStyle, &'static str)>,
}

pub(crate) fn data(vm: &ValidatedModel) -> Data {
    let model = vm.model();
    let mut stores: Vec<PNode> = Vec::new();
    let mut store = |r: &NameRef| -> usize {
        if let Some(i) = stores.iter().position(|s| s.id == r.text) {
            return i;
        }
        let node = match vm.resolved().element(&r.text) {
            Some(id) => element_node(model, id.index()),
            None => PNode::new(&r.text, NodeKind::Store, &r.text),
        };
        stores.push(node);
        stores.len() - 1
    };
    let mut cards = Vec::new();
    let mut links = Vec::new();
    for (c, entity) in model.data.iter().enumerate() {
        let sublabel = match &entity.placement {
            Placement::In { store: s, cached } => {
                links.push((c, store(s), EdgeStyle::Sync, "source of truth"));
                for cache in cached {
                    links.push((c, store(cache), EdgeStyle::Reply, "cache"));
                }
                let mut text = format!("in {}", s.text);
                if !cached.is_empty() {
                    let names: Vec<_> = cached.iter().map(|n| n.text.as_str()).collect();
                    text.push_str(&format!(", cached {}", names.join(", ")));
                }
                text
            }
            Placement::Via { queue, store: s } => {
                links.push((c, store(queue), EdgeStyle::Async, "via"));
                links.push((c, store(s), EdgeStyle::Sync, "source of truth"));
                format!("via {} → {}", queue.text, s.text)
            }
        };
        cards.push(PNode {
            badges: if entity.public {
                vec!["pub".to_owned()]
            } else {
                Vec::new()
            },
            sublabel: Some(sublabel),
            source: Some(entity.span),
            fields: entity.fields.iter().map(|f| f.text.clone()).collect(),
            ..PNode::new(
                format!("data:{}", entity.name.text),
                NodeKind::DataCard,
                &entity.name.text,
            )
        });
    }
    Data {
        stores,
        cards,
        links,
    }
}

/// Whether the model describes a library.
pub(crate) fn is_library(model: &Model) -> bool {
    model
        .roots
        .first()
        .is_some_and(|r| r.kind == RootKind::Library)
}

/// The system's or library's name, for titles.
pub(crate) fn system_name(model: &Model) -> &str {
    model
        .roots
        .first()
        .map_or("system", |s| s.name.text.as_str())
}
