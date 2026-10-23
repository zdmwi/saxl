//! Geometry for each 2D view. Labels use a monospace font in the viewer, so
//! widths are exact: `chars × 0.6 × font_size`.

pub(crate) mod context;
pub(crate) mod data;
pub(crate) mod deployment;
pub(crate) mod layered;
pub(crate) mod runtime;
pub(crate) mod tower;

use crate::project::PNode;
use crate::scene::{NodeKind, Rect, SceneNode};

/// Canvas margin.
pub(crate) const PAD: f64 = 24.0;
/// Title font size.
pub(crate) const TITLE_PX: f64 = 14.0;
/// Sublabel, field, and badge font size.
pub(crate) const SMALL_PX: f64 = 12.0;
/// Horizontal padding inside a node.
const NODE_PAD: f64 = 16.0;
/// Minimum node width.
const MIN_W: f64 = 120.0;
/// Height of the badge row.
pub(crate) const BADGE_ROW: f64 = 20.0;
/// Height of one field line in a data card.
pub(crate) const FIELD_LINE: f64 = 18.0;
/// Vertical radius of a store cylinder's top and bottom ellipses.
pub(crate) const STORE_CAP: f64 = 8.0;
/// Horizontal radius of a queue pipe's end ellipses.
pub(crate) const QUEUE_CAP: f64 = 10.0;

/// How far a node's text starts below its top edge. A store's text sits
/// below the cylinder's top ellipse.
pub(crate) fn content_top(kind: NodeKind) -> f64 {
    if kind == NodeKind::Store {
        1.5 * STORE_CAP
    } else {
        0.0
    }
}

/// How far a node's centered text sits left of its center. A queue's visible
/// face ends where its right end's rim curves in.
pub(crate) fn content_shift(kind: NodeKind) -> f64 {
    if kind == NodeKind::Queue {
        QUEUE_CAP / 2.0
    } else {
        0.0
    }
}

/// Width of `text` at `px`.
pub(crate) fn text_width(text: &str, px: f64) -> f64 {
    text.chars().count() as f64 * 0.6 * px
}

/// The text of a node's badge row.
pub(crate) fn badge_text(badges: &[String]) -> String {
    badges.join("  ")
}

/// Width and height of a node from its labels, badges, and fields.
pub(crate) fn node_size(n: &PNode) -> (f64, f64) {
    let sub = n
        .sublabel
        .as_deref()
        .map_or(0.0, |s| text_width(s, SMALL_PX));
    let badges = text_width(&badge_text(&n.badges), SMALL_PX);
    let fields = n
        .fields
        .iter()
        .map(|f| text_width(f, SMALL_PX))
        .fold(0.0, f64::max);
    let w = text_width(&n.label, TITLE_PX)
        .max(sub)
        .max(badges)
        .max(fields)
        + 2.0 * NODE_PAD;
    let mut h = if n.sublabel.is_some() { 56.0 } else { 40.0 };
    if !n.badges.is_empty() {
        h += BADGE_ROW;
    }
    if !n.fields.is_empty() {
        h += n.fields.len() as f64 * FIELD_LINE + 8.0;
    }
    // Room for a cylinder's top and bottom ellipses, or a pipe's ends.
    let (w, h) = match n.kind {
        NodeKind::Store => (w, h + 2.0 * STORE_CAP),
        NodeKind::Queue => (w + 3.0 * QUEUE_CAP, h),
        _ => (w, h),
    };
    (w.max(MIN_W), h)
}

/// Places a projected node.
pub(crate) fn place(n: PNode, rect: Rect) -> SceneNode {
    SceneNode {
        id: n.id,
        kind: n.kind,
        label: n.label,
        sublabel: n.sublabel,
        rect,
        source: n.source,
        badges: n.badges,
        fields: n.fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::NodeKind;

    fn node(label: &str, sub: Option<&str>) -> PNode {
        PNode {
            id: label.to_owned(),
            kind: NodeKind::Service,
            label: label.to_owned(),
            sublabel: sub.map(str::to_owned),
            source: None,
            badges: Vec::new(),
            fields: Vec::new(),
        }
    }

    #[test]
    fn label_metrics() {
        assert_eq!(node_size(&node("A", None)), (120.0, 40.0));
        // 20 chars × 0.6 × 12 = 144, + 32 padding.
        let (w, h) = node_size(&node("A", Some("aggregates stats xxx")));
        assert!((w - 176.0).abs() < 1e-9);
        assert_eq!(h, 56.0);
    }
}
