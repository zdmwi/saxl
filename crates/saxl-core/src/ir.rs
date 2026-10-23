//! The model IR: the parser's output and the validator's input.

use serde::{Deserialize, Serialize};

use crate::Span;

/// Version of the IR's JSON shape. Bump on breaking changes.
pub const IR_VERSION: u32 = 2;

/// A name at its declaration site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Name {
    /// The identifier.
    pub text: String,
    /// Where it was written.
    pub span: Span,
}

/// A name at a use site, not yet resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NameRef {
    /// The identifier.
    pub text: String,
    /// Where it was written.
    pub span: Span,
}

/// A whole saxl file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Model {
    /// Always [`IR_VERSION`] for models built by this version.
    pub ir_version: u32,
    /// Every `system` and `library` statement. The validator requires exactly one (E003).
    pub roots: Vec<Root>,
    /// Element declarations, in source order.
    pub elements: Vec<Element>,
    /// Relations, with chains expanded and duplicates merged.
    pub relations: Vec<Relation>,
    /// Data entities.
    pub data: Vec<DataEntity>,
    /// Deployment regions.
    pub regions: Vec<Region>,
    /// `publish` registries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registries: Vec<Registry>,
    /// `impl` statements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impls: Vec<Impl>,
    /// Scenarios.
    pub scenarios: Vec<Scenario>,
    /// `allow` statements.
    pub allows: Vec<Allow>,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            ir_version: IR_VERSION,
            roots: Vec::new(),
            elements: Vec::new(),
            relations: Vec::new(),
            data: Vec::new(),
            regions: Vec::new(),
            registries: Vec::new(),
            impls: Vec::new(),
            scenarios: Vec::new(),
            allows: Vec::new(),
        }
    }
}

/// What a file describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum RootKind {
    /// A deployable system.
    System,
    /// A library: code other code calls.
    Library,
}

impl RootKind {
    /// The keyword that declares this kind.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Library => "library",
        }
    }
}

/// The system or library boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Root {
    /// System or library.
    pub kind: RootKind,
    /// The root's name.
    pub name: Name,
    /// Optional description.
    pub description: Option<String>,
    /// The whole statement.
    pub span: Span,
}

/// The kind of an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ElementKind {
    /// A person or role using the system.
    Actor,
    /// A third-party system.
    External,
    /// Runnable code.
    Service,
    /// Persistent or cached state.
    Store,
    /// A message broker, stream, or topic.
    Queue,
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
}

impl ElementKind {
    /// Whether elements of this kind are inside the system or library boundary.
    pub fn is_internal(self) -> bool {
        matches!(
            self,
            Self::Service | Self::Store | Self::Queue | Self::Api | Self::Module
        )
    }

    /// `a` or `an`, whichever goes before [`Self::keyword`].
    pub fn article(self) -> &'static str {
        match self {
            Self::Actor | Self::External | Self::Api => "an",
            _ => "a",
        }
    }

    /// The keyword that declares this kind.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Actor => "actor",
            Self::External => "external",
            Self::Service => "service",
            Self::Store => "store",
            Self::Queue => "queue",
            Self::Consumer => "consumer",
            Self::Package => "package",
            Self::Api => "api",
            Self::Module => "module",
            Self::Hook => "hook",
        }
    }
}

/// A declared element: an actor, service, module, hook, and so on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Element {
    /// The element's name.
    pub name: Name,
    /// What it is.
    pub kind: ElementKind,
    /// Optional description.
    pub description: Option<String>,
    /// `at "glob"`: where the element's code lives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Glob>,
    /// The whole statement.
    pub span: Span,
}

/// A path pattern such as `src/blocks/**`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Glob {
    /// The pattern, without quotes.
    pub text: String,
    /// Where it was written, including quotes.
    pub span: Span,
}

/// Whether a caller waits for a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Style {
    /// `->`: the caller waits.
    Sync,
    /// `~>`: fire and forget.
    Async,
}

impl Style {
    /// The arrow that declares this style.
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Sync => "->",
            Self::Async => "~>",
        }
    }
}

/// One directed relation between two elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Relation {
    /// The caller or sender.
    pub from: NameRef,
    /// The callee or receiver.
    pub to: NameRef,
    /// Sync or async.
    pub style: Style,
    /// Optional description. Only single-hop relations have one.
    pub description: Option<String>,
    /// From the start of `from` to the end of `to`.
    pub span: Span,
}

/// A data entity and where it lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DataEntity {
    /// The entity's name.
    pub name: Name,
    /// Where it lives.
    pub placement: Placement,
    /// Field names.
    pub fields: Vec<Name>,
    /// `pub`: part of a library's public API.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub public: bool,
    /// The whole statement.
    pub span: Span,
}

/// Where a data entity lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Placement {
    /// `in store cached a, b`.
    In {
        /// The source of truth.
        store: NameRef,
        /// Stores holding copies.
        cached: Vec<NameRef>,
    },
    /// `via queue -> store`.
    Via {
        /// The queue the data flows through.
        queue: NameRef,
        /// Where the data comes to rest; its source of truth.
        store: NameRef,
    },
}

/// A deployment region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Region {
    /// The region's name.
    pub name: Name,
    /// Infrastructure nodes declared in the region.
    pub infra: Vec<InfraNode>,
    /// Clusters and managed groups.
    pub groups: Vec<Group>,
    /// The whole block.
    pub span: Span,
}

/// `publish registry { … }`: where a library ships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Registry {
    /// The registry's name, such as `crates.io` or `npm`.
    pub name: Name,
    /// Packages published there.
    pub packages: Vec<Package>,
    /// The whole block.
    pub span: Span,
}

/// The keyword that declares a published package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PackageKind {
    /// `crate`.
    Crate,
    /// `package`.
    Package,
}

/// One published package and the elements it contains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Package {
    /// Crate or package.
    pub kind: PackageKind,
    /// The package's name, such as `quill` or `@quill/wasm`.
    pub name: Name,
    /// `target t`: the platform it is built for.
    pub target: Option<Name>,
    /// Workload lines.
    pub workloads: Vec<Workload>,
    /// The whole block.
    pub span: Span,
}

/// `implementor impl hook`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Impl {
    /// The module or consumer providing the implementation.
    pub implementor: NameRef,
    /// The hook implemented.
    pub hook: NameRef,
    /// The whole statement.
    pub span: Span,
}

/// The kind of an infrastructure node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum InfraKind {
    /// A content delivery network edge.
    Cdn,
}

/// An infrastructure node such as a CDN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct InfraNode {
    /// The node's name.
    pub name: Name,
    /// What it is.
    pub kind: InfraKind,
    /// The whole line.
    pub span: Span,
}

/// The kind of a deployment group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum GroupKind {
    /// A named compute cluster.
    Cluster,
    /// Managed services.
    Managed,
}

/// A cluster or managed group inside a region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Group {
    /// Cluster or managed.
    pub kind: GroupKind,
    /// The cluster's name. Managed groups have none.
    pub name: Option<Name>,
    /// Workload lines.
    pub workloads: Vec<Workload>,
    /// The whole block.
    pub span: Span,
}

/// One workload line: names and the attributes that apply to all of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Workload {
    /// The deployed elements.
    pub names: Vec<NameRef>,
    /// Attributes applying to every name.
    pub attributes: Vec<Attribute>,
    /// The whole line.
    pub span: Span,
}

impl Workload {
    /// The replica count, defaulting to 1.
    pub fn replicas(&self) -> u32 {
        self.attributes
            .iter()
            .find_map(|a| match a {
                Attribute::Replicas { count, .. } => Some(*count),
                _ => None,
            })
            .unwrap_or(1)
    }

    /// Whether the line has `autoscale`.
    pub fn autoscale(&self) -> bool {
        self.attributes
            .iter()
            .any(|a| matches!(a, Attribute::Autoscale { .. }))
    }

    /// The feature named by `feature f`, if the line has one.
    pub fn feature(&self) -> Option<&Name> {
        self.attributes.iter().find_map(|a| match a {
            Attribute::Feature { name, .. } => Some(name),
            _ => None,
        })
    }

    /// Whether the line has `primary+replica`.
    pub fn primary_replica(&self) -> bool {
        self.attributes
            .iter()
            .any(|a| matches!(a, Attribute::PrimaryReplica { .. }))
    }
}

/// A deployment attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Attribute {
    /// `xN`.
    Replicas {
        /// N.
        count: u32,
        /// Where it was written.
        span: Span,
    },
    /// `autoscale`.
    Autoscale {
        /// Where it was written.
        span: Span,
    },
    /// `primary+replica`.
    PrimaryReplica {
        /// Where it was written.
        span: Span,
    },
    /// `feature f`: built only when feature `f` is on.
    Feature {
        /// The feature's name.
        name: Name,
        /// Where it was written.
        span: Span,
    },
    /// Anything else (W108).
    Unknown {
        /// The attribute as written.
        text: String,
        /// Where it was written.
        span: Span,
    },
}

impl Attribute {
    /// Where the attribute was written.
    pub fn span(&self) -> Span {
        match self {
            Self::Replicas { span, .. }
            | Self::Autoscale { span }
            | Self::PrimaryReplica { span }
            | Self::Feature { span, .. }
            | Self::Unknown { span, .. } => *span,
        }
    }
}

/// An arrow in a scenario step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum StepArrow {
    /// `->`: opens a call.
    Call,
    /// `~>`: sends a message.
    Async,
    /// `-->`: replies to an open call.
    Reply,
}

impl StepArrow {
    /// The arrow as written.
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Call => "->",
            Self::Async => "~>",
            Self::Reply => "-->",
        }
    }
}

/// An ordered interaction flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Scenario {
    /// The scenario's name.
    pub name: Name,
    /// Steps, in order.
    pub steps: Vec<Step>,
    /// The whole block.
    pub span: Span,
}

/// A scenario step.
///
/// Invariant: for `Message`, `hops.len() == arrows.len() + 1` and `hops.len() >= 2`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Step {
    /// `A -> B -> C : label`.
    Message {
        /// Participants, in order.
        hops: Vec<NameRef>,
        /// The arrow between each consecutive pair of hops.
        arrows: Vec<StepArrow>,
        /// Optional label.
        label: Option<String>,
        /// The whole line.
        span: Span,
    },
    /// `parallel { … }`.
    Parallel {
        /// Steps that run concurrently.
        steps: Vec<Step>,
        /// The whole block.
        span: Span,
    },
}

impl Step {
    /// Where the step was written.
    pub fn span(&self) -> Span {
        match self {
            Self::Message { span, .. } | Self::Parallel { span, .. } => *span,
        }
    }
}

/// `allow CODE "reason"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Allow {
    /// The rule code, for example `W102`.
    pub code: String,
    /// Where the code was written.
    pub code_span: Span,
    /// Why the rule is allowed.
    pub reason: String,
    /// The whole statement.
    pub span: Span,
}
