//! Runtime view: a sequence diagram. One column per participant in order of
//! first appearance, one row per element-to-element hop. Rows in a `parallel`
//! block share a playback slot and are bracketed by a group. A hook's column
//! is tinted: steps into it run code the library does not own.

use crate::layout::{PAD, node_size, place};
use crate::project::Runtime;
use crate::scene::{GroupKind, NodeKind, Point2, Rect, SceneEdge, SceneGroup, SceneNode, Size2};

/// Distance between participant columns.
const COLUMN: f64 = 180.0;
/// Distance between rows.
const ROW: f64 = 44.0;
/// Space between the participant boxes and the first row.
const HEADER_GAP: f64 = 32.0;
/// Width of a self-message loop.
const LOOP_W: f64 = 40.0;

pub(crate) struct Layout {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
    pub groups: Vec<SceneGroup>,
    pub size: Size2,
}

pub(crate) fn layout(p: Runtime) -> Layout {
    let sizes: Vec<(f64, f64)> = p.participants.iter().map(node_size).collect();
    let widest = sizes.iter().map(|s| s.0).fold(0.0, f64::max);
    let tallest = sizes.iter().map(|s| s.1).fold(0.0, f64::max);
    let spacing = COLUMN.max(widest + 24.0);
    // Leave room for parallel brackets on the left.
    let left = PAD + 16.0;
    let center = |i: usize| left + widest / 2.0 + spacing * i as f64;
    let row_y = |r: usize| PAD + tallest + HEADER_GAP + ROW * r as f64;

    let ids: Vec<String> = p.participants.iter().map(|n| n.id.clone()).collect();
    let nodes: Vec<SceneNode> = p
        .participants
        .into_iter()
        .zip(&sizes)
        .enumerate()
        .map(|(i, (n, (w, h)))| place(n, Rect::new(center(i) - w / 2.0, PAD + tallest - h, *w, *h)))
        .collect();

    let mut edges = Vec::new();
    for (r, row) in p.rows.iter().enumerate() {
        let y = row_y(r);
        let last = row.hops.len().saturating_sub(1);
        for (h, &(from, to, style)) in row.hops.iter().enumerate() {
            let (x1, x2) = (center(from), center(to));
            let points = if from == to {
                vec![
                    Point2::new(x1, y - 8.0),
                    Point2::new(x1 + LOOP_W, y - 8.0),
                    Point2::new(x1 + LOOP_W, y + 8.0),
                    Point2::new(x1, y + 8.0),
                ]
            } else {
                vec![Point2::new(x1, y), Point2::new(x2, y)]
            };
            edges.push(SceneEdge {
                from: ids[from].clone(),
                to: ids[to].clone(),
                style,
                points,
                label: if h == last { row.label.clone() } else { None },
            });
        }
    }

    let width = center(ids.len().saturating_sub(1)) + widest / 2.0 + PAD + LOOP_W;
    let mut groups: Vec<SceneGroup> = p
        .parallels
        .iter()
        .enumerate()
        .map(|(i, &(first, last, depth))| {
            let inset = 8.0 * depth as f64;
            // Room above the first row for its label, which sits above the arrow.
            let top = row_y(first) - ROW / 2.0 - 6.0 + inset / 2.0;
            let bottom = row_y(last) + ROW / 2.0 - 8.0 - inset / 2.0;
            SceneGroup {
                id: format!("parallel-{i}"),
                kind: GroupKind::Parallel,
                label: "parallel".to_owned(),
                rect: Rect::new(
                    PAD + inset,
                    top,
                    width - 2.0 * PAD - 2.0 * inset,
                    bottom - top,
                ),
                depth,
            }
        })
        .collect();

    let height = if p.rows.is_empty() {
        PAD + tallest + PAD
    } else {
        row_y(p.rows.len() - 1) + ROW / 2.0 + PAD
    };
    let zones = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == NodeKind::Hook)
        .map(|(i, n)| SceneGroup {
            id: format!("hook-{}", n.id),
            kind: GroupKind::Hook,
            // Unlabeled: a caption would collide with arrow labels, which
            // sit at column boundaries.
            label: String::new(),
            rect: Rect::new(
                center(i) - spacing / 2.0 + 8.0,
                PAD / 2.0,
                spacing - 16.0,
                height - PAD,
            ),
            depth: 0,
        })
        .collect::<Vec<_>>();
    groups.splice(0..0, zones);
    Layout {
        nodes,
        edges,
        groups,
        size: Size2 {
            w: width,
            h: height,
        },
    }
}
