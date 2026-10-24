//! The scene: geometry for one view, shared with the TypeScript viewer.
//!
//! Units are pixels for 2D views. Every list is in declaration order and every
//! float is rounded to 2 decimal places when serialized, so output is
//! byte-for-byte deterministic.

use saxl_core::Span;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Rounds to 2 decimal places, avoiding `-0`.
fn round2(v: f64) -> f64 {
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 { 0.0 } else { r }
}

/// Serializes a struct of `f64` fields with every value rounded.
fn rounded<S: Serializer>(
    s: S,
    name: &'static str,
    fields: &[(&'static str, f64)],
) -> Result<S::Ok, S::Error> {
    let mut st = s.serialize_struct(name, fields.len())?;
    for (key, value) in fields {
        st.serialize_field(key, &round2(*value))?;
    }
    st.end()
}

impl Serialize for Size2 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        rounded(s, "Size2", &[("w", self.w), ("h", self.h)])
    }
}

impl Serialize for Point2 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        rounded(s, "Point2", &[("x", self.x), ("y", self.y)])
    }
}

impl Serialize for Rect {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        rounded(
            s,
            "Rect",
            &[("x", self.x), ("y", self.y), ("w", self.w), ("h", self.h)],
        )
    }
}

/// Which view a scene shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ViewKind {
    /// The system as one box with its actors and externals.
    Context,
    /// Services, stores, and queues with their relations.
    Containers,
    /// One scenario as a sequence diagram.
    Runtime {
        /// The scenario's name.
        scenario: String,
    },
    /// Data entities and where they live.
    Data,
    /// Regions, clusters, and workloads.
    Deployment,
    /// The 3D view combining all others.
    Tower,
}

impl ViewKind {
    /// A short name, used in CLI arguments and file names.
    pub fn slug(&self) -> String {
        match self {
            Self::Context => "context".to_owned(),
            Self::Containers => "containers".to_owned(),
            Self::Runtime { scenario } => format!("runtime-{scenario}"),
            Self::Data => "data".to_owned(),
            Self::Deployment => "deployment".to_owned(),
            Self::Tower => "tower".to_owned(),
        }
    }
}

/// A 2D size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Size2 {
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// A 2D point.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Point2 {
    /// X, growing right.
    pub x: f64,
    /// Y, growing down.
    pub y: f64,
}

impl Point2 {
    /// A point at `(x, y)`.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned rectangle. `(x, y)` is the top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Rect {
    /// A rectangle from its top-left corner and size.
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// The center point.
    pub fn center(&self) -> Point2 {
        Point2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Right edge.
    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    /// Where the segment from the center towards `p` leaves the rectangle.
    /// Returns the center if `p` is the center.
    pub fn clip_towards(&self, p: Point2) -> Point2 {
        let c = self.center();
        let (dx, dy) = (p.x - c.x, p.y - c.y);
        if dx == 0.0 && dy == 0.0 {
            return c;
        }
        let tx = if dx == 0.0 {
            f64::INFINITY
        } else {
            (self.w / 2.0) / dx.abs()
        };
        let ty = if dy == 0.0 {
            f64::INFINITY
        } else {
            (self.h / 2.0) / dy.abs()
        };
        let t = tx.min(ty).min(1.0);
        Point2::new(c.x + dx * t, c.y + dy * t)
    }
}

/// What a node depicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum NodeKind {
    /// A person or role.
    Actor,
    /// A third-party system.
    External,
    /// Runnable code.
    Service,
    /// Persistent or cached state.
    Store,
    /// A broker, stream, or topic.
    Queue,
    /// The whole system, in the context view.
    System,
    /// An infrastructure node such as a CDN.
    Infra,
    /// A data entity card.
    DataCard,
    /// A region or group slab in the tower's floor.
    Floor,
    /// Code that calls a library.
    Consumer,
    /// A library's third-party dependency.
    Package,
    /// A library's public entry point.
    Api,
    /// A library's internal component.
    Module,
    /// An interface a library calls and others implement.
    Hook,
    /// The whole library, in the context view.
    Library,
}

impl NodeKind {
    /// Kinds the layered layout pins to its first layer: callers from outside.
    pub fn is_source(self) -> bool {
        matches!(self, Self::Actor | Self::Consumer)
    }

    /// Kinds the layered layout pins to its last layer: things called from
    /// inside that live outside.
    pub fn is_sink(self) -> bool {
        matches!(self, Self::External | Self::Package | Self::Hook)
    }
}

impl From<saxl_core::ElementKind> for NodeKind {
    fn from(kind: saxl_core::ElementKind) -> Self {
        use saxl_core::ElementKind as K;
        match kind {
            K::Actor => Self::Actor,
            K::External => Self::External,
            K::Service => Self::Service,
            K::Store => Self::Store,
            K::Queue => Self::Queue,
            K::Consumer => Self::Consumer,
            K::Package => Self::Package,
            K::Api => Self::Api,
            K::Module => Self::Module,
            K::Hook => Self::Hook,
        }
    }
}

/// A box in a 2D view.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SceneNode {
    /// Unique within the scene. The element or infrastructure name, prefixed
    /// where a view can show the same name twice (`data:`, `region/`).
    pub id: String,
    /// What the node depicts.
    pub kind: NodeKind,
    /// The title.
    pub label: String,
    /// The description.
    pub sublabel: Option<String>,
    /// Geometry.
    pub rect: Rect,
    /// Declaration span, for hover and jump-to-source.
    pub source: Option<Span>,
    /// Small tags such as `×3` or `autoscale`.
    pub badges: Vec<String>,
    /// Field names. Data cards only.
    pub fields: Vec<String>,
}

/// How an edge is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum EdgeStyle {
    /// A synchronous call (solid).
    Sync,
    /// An asynchronous message (dashed).
    Async,
    /// A reply (dotted).
    Reply,
    /// An implementation of a hook (dashed, hollow arrowhead).
    Impl,
}

impl From<saxl_core::Style> for EdgeStyle {
    fn from(style: saxl_core::Style) -> Self {
        match style {
            saxl_core::Style::Sync => Self::Sync,
            saxl_core::Style::Async => Self::Async,
        }
    }
}

impl From<saxl_core::StepArrow> for EdgeStyle {
    fn from(arrow: saxl_core::StepArrow) -> Self {
        match arrow {
            saxl_core::StepArrow::Call => Self::Sync,
            saxl_core::StepArrow::Async => Self::Async,
            saxl_core::StepArrow::Reply => Self::Reply,
        }
    }
}

/// A directed line between two nodes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SceneEdge {
    /// Source node id.
    pub from: String,
    /// Target node id.
    pub to: String,
    /// How to draw it.
    pub style: EdgeStyle,
    /// Polyline; first and last points lie on node borders.
    pub points: Vec<Point2>,
    /// Optional text.
    pub label: Option<String>,
}

/// What a group box depicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum GroupKind {
    /// A deployment region.
    Region,
    /// A compute cluster.
    Cluster,
    /// A managed-services group.
    Managed,
    /// A `parallel` block in the runtime view.
    Parallel,
    /// A registry in a library's packages view.
    Registry,
    /// A published package.
    Package,
    /// A hook's column in the runtime view: code the library does not own.
    Hook,
}

/// A labeled box drawn behind nodes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SceneGroup {
    /// Unique within the scene.
    pub id: String,
    /// What the box depicts.
    pub kind: GroupKind,
    /// Caption.
    pub label: String,
    /// Geometry.
    pub rect: Rect,
    /// Nesting depth; 0 is outermost.
    pub depth: u32,
}

/// The ordered steps of one scenario, for playback.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Timeline {
    /// The scenario's name.
    pub scenario: String,
    /// Steps in playback order.
    pub steps: Vec<TimelineStep>,
}

/// One element-to-element hop of a scenario.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TimelineStep {
    /// Playback slot. Steps in a `parallel` block share a slot.
    pub index: u32,
    /// The sending element.
    pub from: String,
    /// The receiving element.
    pub to: String,
    /// Infrastructure nodes passed through, in order.
    pub via: Vec<String>,
    /// How the hop is drawn.
    pub arrow: EdgeStyle,
    /// The step's label.
    pub label: Option<String>,
    /// Row in the runtime view.
    pub row: Option<u32>,
}

/// Everything the viewer needs to draw one view.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Scene {
    /// Which view this is.
    pub view: ViewKind,
    /// Title shown above the view.
    pub title: String,
    /// 2D canvas size.
    pub size: Size2,
    /// Boxes.
    pub nodes: Vec<SceneNode>,
    /// Lines between boxes.
    pub edges: Vec<SceneEdge>,
    /// Boundaries, regions, clusters, and parallel brackets.
    pub groups: Vec<SceneGroup>,
    /// The 3D geometry. Tower only.
    pub tower: Option<Tower>,
    /// Runtime: the scenario's steps. Tower: one per scenario.
    pub timelines: Vec<Timeline>,
    /// Every source span that names something drawn in this scene, so the
    /// viewer can mark the nodes a diagnostic's primary label points at.
    pub anchors: Vec<Anchor>,
}

/// A source span that names the node (or 3D box) with id `node`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Anchor {
    /// The node or box id.
    pub node: String,
    /// A declaration or reference of the element it depicts.
    pub span: Span,
}

/// A 3D point. World units, y up.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Point3 {
    /// X.
    pub x: f64,
    /// Y, up.
    pub y: f64,
    /// Z.
    pub z: f64,
}

impl Point3 {
    /// A point at `(x, y, z)`.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl Serialize for Point3 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        rounded(s, "Point3", &[("x", self.x), ("y", self.y), ("z", self.z)])
    }
}

/// An axis-aligned box.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Box3 {
    /// Unique within its list. Element and infrastructure boxes use the name.
    pub id: String,
    /// What the box depicts.
    pub kind: NodeKind,
    /// Center.
    pub center: Point3,
    /// Full extent along each axis.
    pub size: Point3,
    /// Title, drawn as a sprite.
    pub label: Option<String>,
    /// Description.
    pub sublabel: Option<String>,
}

/// A straight segment.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Segment3 {
    /// Start.
    pub from: Point3,
    /// End.
    pub to: Point3,
}

/// A floating caption.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Label3 {
    /// The text.
    pub text: String,
    /// Where it sits.
    pub position: Point3,
}

/// One replica (a pod on a cluster) or one managed instance, on the floor.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Pod {
    /// The element it runs.
    pub element: String,
    /// Managed instances are flat pads; cluster replicas are cubes.
    pub managed: bool,
    /// Geometry.
    pub geometry: Box3,
}

/// A caption on the floor that belongs to an element, such as `×3`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Tag {
    /// The element it describes.
    pub element: String,
    /// The text.
    pub text: String,
    /// Where it sits.
    pub position: Point3,
}

/// A dashed line from a pod up to the container it runs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Leader {
    /// The element.
    pub element: String,
    /// The pod's top, on the floor.
    pub from: Point3,
    /// The container's underside.
    pub to: Point3,
}

/// One suspended level of containers: a deployment group.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Tier {
    /// The group, as `region/group`.
    pub id: String,
    /// Caption, for example `edge cluster`.
    pub label: String,
    /// One corner of the tier's footprint, at the tier's base height.
    pub min: Point3,
    /// The opposite corner, at the same height.
    pub max: Point3,
}

/// One relation drawn as a pipe.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Pipe {
    /// `from>to`, unique per relation.
    pub id: String,
    /// The caller or sender.
    pub from: String,
    /// The callee or receiver.
    pub to: String,
    /// Asynchronous pipes carry coupling rings.
    #[serde(rename = "async")]
    pub is_async: bool,
    /// The tube's center line. A trunk branch starts at its tee on the trunk.
    pub points: Vec<Point3>,
    /// The whole path from `from` to `to`, through the trunk for a branch;
    /// what playback follows.
    pub route: Vec<Point3>,
    /// The hub whose trunk this pipe branches from.
    pub trunk: Option<String>,
    /// Tube radius.
    pub radius: f64,
}

/// A shared pipe leaving a busy hub; its branches are [`Pipe`]s.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Trunk {
    /// The hub element.
    pub hub: String,
    /// From the hub's face out to the lane.
    pub stub: Vec<Point3>,
    /// Along the lane, over every branch's tee.
    pub run: Vec<Point3>,
    /// The ids of the pipes branching from it.
    pub pipes: Vec<String>,
    /// Tube radius.
    pub radius: f64,
}

/// The system boundary: a box around every internal container.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Boundary {
    /// The system's or library's name.
    pub label: String,
    /// `system` or `library`.
    pub kind: NodeKind,
    /// Lower corner.
    pub min: Point3,
    /// Upper corner.
    pub max: Point3,
}

/// The suspended tower: the deployment floor, containers suspended above it
/// in tiers (one per deployment group), each on a dashed leader from its
/// replicas, and relations drawn as pipes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Tower {
    /// Floor slabs: regions and their groups.
    pub slabs: Vec<Box3>,
    /// Floor captions, painted flat.
    pub labels: Vec<Label3>,
    /// One per replica or managed instance.
    pub pods: Vec<Pod>,
    /// Replica counts beside each element's pods.
    pub tags: Vec<Tag>,
    /// Infrastructure nodes on the floor.
    pub infra: Vec<Box3>,
    /// Height of the lowest tier's base. The viewer's explode control moves
    /// everything above the floor relative to it.
    pub level: f64,
    /// The suspended levels, in declaration order: the top tier first.
    pub tiers: Vec<Tier>,
    /// One box per internal element; id is the element name.
    pub containers: Vec<Box3>,
    /// Actors and externals, outside the boundary.
    pub outside: Vec<Box3>,
    /// From each pod up to its container.
    pub leaders: Vec<Leader>,
    /// One per relation.
    pub pipes: Vec<Pipe>,
    /// Shared trunks of busy hubs.
    pub trunks: Vec<Trunk>,
    /// The system boundary.
    pub boundary: Boundary,
    /// Pairs of unrelated pipes the router could not keep apart. Zero when
    /// every pipe clears every other.
    pub clashes: u32,
}
