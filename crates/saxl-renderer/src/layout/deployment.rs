//! Deployment view: nested box packing. Workloads stack vertically in their
//! group, groups sit side by side in their region, and regions stack
//! vertically. Infrastructure nodes sit in a column on the region's left edge.

use crate::layout::{PAD, node_size, place};
use crate::project::{DeployRegion, PNode};
use crate::scene::{GroupKind, Rect, SceneGroup, SceneNode, Size2};

/// Padding inside every box.
const INSET: f64 = 16.0;
/// Header height for a box's label.
const HEADER: f64 = 28.0;
const GAP: f64 = 16.0;
const REGION_GAP: f64 = 24.0;
const MIN_GROUP_W: f64 = 120.0;

pub(crate) struct Layout {
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SceneGroup>,
    pub size: Size2,
}

/// Stacks nodes in a column at `(x, y)`. Returns the column's size.
fn column(nodes: Vec<PNode>, x: f64, y: f64, out: &mut Vec<SceneNode>) -> (f64, f64) {
    let width = nodes.iter().map(|n| node_size(n).0).fold(0.0, f64::max);
    let mut cy = y;
    for n in nodes {
        let (w, h) = node_size(&n);
        out.push(place(n, Rect::new(x, cy, w.max(width), h)));
        cy += h + GAP;
    }
    (width, (cy - y - GAP).max(0.0))
}

pub(crate) fn layout(regions: Vec<DeployRegion>) -> Layout {
    let mut nodes = Vec::new();
    let mut groups = Vec::new();
    let mut y = PAD;
    let mut max_right = PAD;
    for region in regions {
        let region_index = groups.len();
        let top = y + HEADER;
        let mut x = PAD + INSET;
        let mut content_h: f64 = 0.0;
        if !region.infra.is_empty() {
            let (w, h) = column(region.infra, x, top + HEADER, &mut nodes);
            x += w + GAP;
            content_h = content_h.max(HEADER + h);
        }
        for group in region.groups {
            let first = nodes.len();
            let (w, h) = column(group.nodes, x + INSET, top + HEADER, &mut nodes);
            let gw = (w + 2.0 * INSET).max(MIN_GROUP_W);
            // Center narrow columns in the minimum group width.
            let shift = (gw - 2.0 * INSET - w) / 2.0;
            for n in &mut nodes[first..] {
                n.rect.x += shift;
            }
            let gh = HEADER + h + INSET;
            groups.push(SceneGroup {
                id: group.id,
                kind: group.kind,
                label: group.label,
                rect: Rect::new(x, top, gw, gh),
                depth: 1,
            });
            x += gw + GAP;
            content_h = content_h.max(gh);
        }
        let right = (x - GAP + INSET).max(PAD + MIN_GROUP_W);
        let rect = Rect::new(PAD, y, right - PAD, HEADER + content_h + INSET);
        groups.insert(
            region_index,
            SceneGroup {
                id: region.id,
                kind: GroupKind::Region,
                label: region.label,
                rect,
                depth: 0,
            },
        );
        max_right = max_right.max(rect.right());
        y = rect.bottom() + REGION_GAP;
    }
    Layout {
        nodes,
        groups,
        size: Size2 {
            w: max_right + PAD,
            h: (y - REGION_GAP).max(PAD) + PAD,
        },
    }
}
