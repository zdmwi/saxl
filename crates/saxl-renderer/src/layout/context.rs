//! Context view: actors stacked on the left, the system box in the middle,
//! externals stacked on the right.

use crate::layout::{PAD, node_size, place};
use crate::project::{Context, PNode};
use crate::scene::{Point2, Rect, SceneEdge, SceneNode, Size2};

const COLUMN_GAP: f64 = 80.0;
const NODE_GAP: f64 = 24.0;
const MIN_SYSTEM_W: f64 = 240.0;
const MIN_SYSTEM_H: f64 = 120.0;

pub(crate) struct Layout {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
    pub size: Size2,
}

/// Total height of a stacked column.
fn column_height(nodes: &[PNode]) -> f64 {
    let sum: f64 = nodes.iter().map(|n| node_size(n).1).sum();
    sum + NODE_GAP * nodes.len().saturating_sub(1) as f64
}

fn column_width(nodes: &[PNode]) -> f64 {
    nodes.iter().map(|n| node_size(n).0).fold(0.0, f64::max)
}

pub(crate) fn layout(p: Context) -> Layout {
    let (sw, sh) = node_size(&p.system);
    let height = column_height(&p.actors)
        .max(column_height(&p.externals))
        .max(sh)
        .max(MIN_SYSTEM_H);

    let actors_w = column_width(&p.actors);
    let system_x = if p.actors.is_empty() {
        PAD
    } else {
        PAD + actors_w + COLUMN_GAP
    };
    let system = Rect::new(system_x, PAD, sw.max(MIN_SYSTEM_W), height);
    let externals_x = system.right() + COLUMN_GAP;
    let externals_w = column_width(&p.externals);

    let mut nodes = Vec::new();
    let stack = |column: Vec<PNode>, x: f64, w: f64, nodes: &mut Vec<SceneNode>| {
        let mut y = PAD + (height - column_height(&column)) / 2.0;
        for n in column {
            let (nw, nh) = node_size(&n);
            let rect = Rect::new(x + (w - nw) / 2.0, y, nw, nh);
            y += nh + NODE_GAP;
            nodes.push(place(n, rect));
        }
    };
    stack(p.actors, PAD, actors_w, &mut nodes);
    stack(p.externals, externals_x, externals_w, &mut nodes);

    let mut edges = Vec::new();
    for (id, outbound, style) in p.links {
        let Some(node) = nodes.iter().find(|n| n.id == id) else {
            continue;
        };
        let y = node.rect.center().y;
        let left_of_system = node.rect.x < system.x;
        let (outer, inner) = if left_of_system {
            (Point2::new(node.rect.right(), y), Point2::new(system.x, y))
        } else {
            (Point2::new(node.rect.x, y), Point2::new(system.right(), y))
        };
        let (from, to, points) = if outbound {
            (id, p.system.id.clone(), vec![outer, inner])
        } else {
            (p.system.id.clone(), id, vec![inner, outer])
        };
        edges.push(SceneEdge {
            from,
            to,
            style,
            points,
            label: None,
        });
    }

    let right = if externals_w > 0.0 {
        externals_x + externals_w
    } else {
        system.right()
    };
    nodes.insert(0, place(p.system, system));
    Layout {
        nodes,
        edges,
        size: Size2 {
            w: right + PAD,
            h: height + 2.0 * PAD,
        },
    }
}
