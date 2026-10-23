//! The rule registry and the helpers rules share.

use saxl_core::{
    Diagnostic, Element, Model, NameRef, RootKind, Scenario, Severity, Span, Step, StepArrow, Style,
};

use crate::resolve::{BrokenRefs, ElementId, Resolved, Target};

mod e001_duplicate_name;
mod e002_undeclared_name;
mod e003_system_count;
mod e004_outside_relation;
mod e005_placement_kind;
mod e006_step_relation;
mod e007_reply_call;
mod e008_outside_deployed;
mod e009_deployed_twice;
mod e010_allow_code;
mod e011_infra_misuse;
mod e012_consumer_internal;
mod e013_impl_kind;
mod e014_at_overlap;
mod e015_wrong_root;
mod i201_single_replica;
mod i202_unimplemented_hook;
mod w101_not_deployed;
mod w102_single_region;
mod w103_store_replica;
mod w104_unused_element;
mod w105_step_style;
mod w106_queue_ends;
mod w107_sync_cycle;
mod w108_unknown_attribute;
mod w109_feature_leak;
mod w110_not_published;

/// A semantic check.
pub trait Rule: Sync {
    /// The rule's code, for example `E006`.
    fn code(&self) -> &'static str;
    /// The severity of every diagnostic the rule reports.
    fn severity(&self) -> Severity;
    /// One line describing the rule, used in `saxl explain` lists.
    fn summary(&self) -> &'static str;
    /// Appends the rule's diagnostics to `out`.
    fn check(&self, cx: &Context<'_>, out: &mut Vec<Diagnostic>);
}

/// What a rule can see.
pub struct Context<'a> {
    /// The model being checked.
    pub model: &'a Model,
    /// Name resolution results.
    pub resolved: &'a Resolved,
    /// Statements to skip because a reference failed to resolve.
    pub broken: &'a BrokenRefs,
}

impl Context<'_> {
    /// Whether the file describes a library. A file with no root is checked
    /// as a system.
    pub fn is_library(&self) -> bool {
        self.model
            .roots
            .first()
            .is_some_and(|r| r.kind == RootKind::Library)
    }

    /// The element with `id`.
    pub fn element(&self, id: ElementId) -> &Element {
        &self.model.elements[id.index()]
    }

    /// Whether a relation `from → to` with `style` is declared.
    pub fn has_relation(&self, from: ElementId, to: ElementId, style: Style) -> bool {
        self.model
            .relations
            .iter()
            .zip(&self.resolved.relation_ends)
            .any(|(r, ends)| r.style == style && *ends == Some((from, to)))
    }

    /// Scenarios whose references all resolved.
    pub fn scenarios(&self) -> impl Iterator<Item = &Scenario> {
        self.model
            .scenarios
            .iter()
            .zip(&self.broken.scenarios)
            .filter(|(_, broken)| !**broken)
            .map(|(s, _)| s)
    }
}

/// One hop of a scenario step between two elements, after infrastructure
/// nodes are removed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Hop {
    pub from: ElementId,
    pub to: ElementId,
    pub arrow: StepArrow,
}

/// Removes infrastructure nodes from a step and pairs up the remaining hops.
/// Each pair takes the arrow that leaves its first hop.
pub(crate) fn element_hops(
    resolved: &Resolved,
    hops: &[NameRef],
    arrows: &[StepArrow],
) -> Vec<Hop> {
    let mut out = Vec::new();
    let mut prev: Option<(ElementId, StepArrow)> = None;
    for (i, hop) in hops.iter().enumerate() {
        let Some(Target::Element(id)) = resolved.lookup(&hop.text) else {
            continue;
        };
        if let Some((from, arrow)) = prev {
            out.push(Hop {
                from,
                to: id,
                arrow,
            });
        }
        if let Some(arrow) = arrows.get(i) {
            prev = Some((id, *arrow));
        }
    }
    out
}

/// Calls `f` for every message step, depth first, in source order.
pub(crate) fn for_each_message(steps: &[Step], f: &mut impl FnMut(&[NameRef], &[StepArrow], Span)) {
    for step in steps {
        match step {
            Step::Message {
                hops, arrows, span, ..
            } => f(hops, arrows, *span),
            Step::Parallel { steps, .. } => for_each_message(steps, f),
        }
    }
}

static RULES: &[&dyn Rule] = &[
    &e001_duplicate_name::Rule,
    &e002_undeclared_name::Rule,
    &e003_system_count::Rule,
    &e004_outside_relation::Rule,
    &e005_placement_kind::Rule,
    &e006_step_relation::Rule,
    &e007_reply_call::Rule,
    &e008_outside_deployed::Rule,
    &e009_deployed_twice::Rule,
    &e010_allow_code::Rule,
    &e011_infra_misuse::Rule,
    &e012_consumer_internal::Rule,
    &e013_impl_kind::Rule,
    &e014_at_overlap::Rule,
    &e015_wrong_root::Rule,
    &w101_not_deployed::Rule,
    &w102_single_region::Rule,
    &w103_store_replica::Rule,
    &w104_unused_element::Rule,
    &w105_step_style::Rule,
    &w106_queue_ends::Rule,
    &w107_sync_cycle::Rule,
    &w108_unknown_attribute::Rule,
    &w109_feature_leak::Rule,
    &w110_not_published::Rule,
    &i201_single_replica::Rule,
    &i202_unimplemented_hook::Rule,
];

/// Every validator rule, in the order they run.
pub fn rules() -> &'static [&'static dyn Rule] {
    RULES
}

/// Copies the resolution diagnostics with `code`.
pub(crate) fn resolution_diagnostics(cx: &Context<'_>, code: &str, out: &mut Vec<Diagnostic>) {
    out.extend(
        cx.broken
            .diagnostics
            .iter()
            .filter(|d| d.code.0 == code)
            .cloned(),
    );
}
