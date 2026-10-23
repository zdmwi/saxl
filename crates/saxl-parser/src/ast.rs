//! The private syntax tree. It mirrors the grammar; `lower` turns it into the IR.

use saxl_core::{Attribute, ElementKind, GroupKind, Name, NameRef, Span, StepArrow, Style};

/// An identifier as written.
#[derive(Debug, Clone)]
pub(crate) struct Ident {
    pub text: String,
    pub span: Span,
}

impl Ident {
    pub fn name(&self) -> Name {
        Name {
            text: self.text.clone(),
            span: self.span,
        }
    }

    pub fn name_ref(&self) -> NameRef {
        NameRef {
            text: self.text.clone(),
            span: self.span,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Stmt {
    System {
        name: Ident,
        description: Option<String>,
        span: Span,
    },
    Element {
        kind: ElementKind,
        name: Ident,
        description: Option<String>,
        span: Span,
    },
    /// `A -> B ~> C "desc"`. `arrows.len() == hops.len() - 1`.
    Relation {
        hops: Vec<Ident>,
        arrows: Vec<Style>,
        description: Option<String>,
    },
    Data {
        name: Ident,
        placement: Placement,
        fields: Vec<Ident>,
        span: Span,
    },
    Deploy {
        name: Ident,
        items: Vec<DeployItem>,
        span: Span,
    },
    Scenario {
        name: Ident,
        steps: Vec<Step>,
        span: Span,
    },
    Allow {
        code: Ident,
        reason: String,
        span: Span,
    },
}

#[derive(Debug)]
pub(crate) enum Placement {
    In { store: Ident, cached: Vec<Ident> },
    Via { queue: Ident, store: Ident },
}

#[derive(Debug)]
pub(crate) enum DeployItem {
    Cdn {
        name: Ident,
        span: Span,
    },
    Group {
        kind: GroupKind,
        name: Option<Ident>,
        workloads: Vec<Workload>,
        span: Span,
    },
}

#[derive(Debug)]
pub(crate) struct Workload {
    pub names: Vec<Ident>,
    pub attributes: Vec<Attribute>,
    pub span: Span,
}

#[derive(Debug)]
pub(crate) enum Step {
    Message {
        hops: Vec<Ident>,
        arrows: Vec<StepArrow>,
        label: Option<String>,
        span: Span,
    },
    Parallel {
        steps: Vec<Step>,
        span: Span,
    },
}
