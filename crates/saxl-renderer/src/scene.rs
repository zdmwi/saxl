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
}

impl From<saxl_core::ElementKind> for NodeKind {
    fn from(kind: saxl_core::ElementKind) -> Self {
        match kind {
            saxl_core::ElementKind::Actor => Self::Actor,
            saxl_core::ElementKind::External => Self::External,
            saxl_core::ElementKind::Service => Self::Service,
            saxl_core::ElementKind::Store => Self::Store,
            saxl_core::ElementKind::Queue => Self::Queue,
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
    /// Runtime: the scenario's steps.
    pub timelines: Vec<Timeline>,
}
