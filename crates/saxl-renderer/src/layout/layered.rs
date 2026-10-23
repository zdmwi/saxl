//! Layered (Sugiyama-style) layout, flowing left to right. Used by the
//! containers view.
//!
//! 1. Break cycles: reverse DFS back edges (and edges into actors or out of
//!    externals), for layout only.
//! 2. Assign layers by longest path. Actors and consumers are pinned to
//!    layer 0; externals, packages, and hooks to the last layer.
//! 3. Insert dummy nodes on edges spanning more than one layer.
//! 4. Reduce crossings with barycenter ordering, four down-and-up sweeps.
//! 5. Assign coordinates, then pull nodes toward the median of their neighbors.
//! 6. Route edges through dummy positions, clipped to node borders.

use crate::layout::{PAD, SMALL_PX, node_size, place, text_width};
use crate::project::{PEdge, PNode};
use crate::scene::{NodeKind, Point2, Rect, SceneEdge, SceneNode, Size2};

const LAYER_GAP: f64 = 80.0;
const NODE_GAP: f64 = 24.0;
const DUMMY_H: f64 = 8.0;
const SWEEPS: usize = 4;

/// One node in the layered graph: a real node or a dummy on a long edge.
struct LNode {
    layer: usize,
    w: f64,
    h: f64,
    /// Tie-breaker: declaration order for real nodes, after them for dummies.
    key: usize,
    x: f64,
    y: f64,
}

/// The output of the layout.
pub(crate) struct Layout {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
    pub size: Size2,
}

pub(crate) fn layout(nodes: Vec<PNode>, edges: &[PEdge]) -> Layout {
    let n = nodes.len();
    let kinds: Vec<NodeKind> = nodes.iter().map(|p| p.kind).collect();

    // 1. Orient edges for layering.
    let mut oriented: Vec<Option<(usize, usize, bool)>> = Vec::with_capacity(edges.len());
    for e in edges {
        if e.from == e.to {
            oriented.push(None);
            continue;
        }
        let reverse = kinds[e.to].is_source() || kinds[e.from].is_sink();
        oriented.push(Some(if reverse {
            (e.to, e.from, true)
        } else {
            (e.from, e.to, false)
        }));
    }
    break_cycles(n, &mut oriented);

    // 2. Layers by longest path.
    let layer = assign_layers(n, &kinds, &oriented);
    let layer_count = layer.iter().copied().max().map_or(0, |m| m + 1);

    let mut lnodes: Vec<LNode> = nodes
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (w, h) = node_size(p);
            LNode {
                layer: layer[i],
                w,
                h,
                key: i,
                x: 0.0,
                y: 0.0,
            }
        })
        .collect();

    // 3. Dummy chains. `chains[e]` lists the layered-graph nodes an edge passes
    //    through in layout direction, endpoints included.
    let mut chains: Vec<Vec<usize>> = Vec::with_capacity(edges.len());
    let mut adjacent: Vec<(usize, usize)> = Vec::new();
    for o in &oriented {
        let Some((u, v, _)) = *o else {
            chains.push(Vec::new());
            continue;
        };
        let mut chain = vec![u];
        for l in layer[u] + 1..layer[v] {
            lnodes.push(LNode {
                layer: l,
                w: 0.0,
                h: DUMMY_H,
                key: n + lnodes.len(),
                x: 0.0,
                y: 0.0,
            });
            chain.push(lnodes.len() - 1);
        }
        chain.push(v);
        for pair in chain.windows(2) {
            adjacent.push((pair[0], pair[1]));
        }
        chains.push(chain);
    }

    // 4. Crossing reduction.
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); layer_count];
    for (i, ln) in lnodes.iter().enumerate() {
        layers[ln.layer].push(i);
    }
    let mut pos = vec![0usize; lnodes.len()];
    let index_positions = |layers: &Vec<Vec<usize>>, pos: &mut Vec<usize>| {
        for layer in layers {
            for (p, &v) in layer.iter().enumerate() {
                pos[v] = p;
            }
        }
    };
    index_positions(&layers, &mut pos);
    let left: Vec<Vec<usize>> = neighbors(lnodes.len(), &adjacent, false);
    let right: Vec<Vec<usize>> = neighbors(lnodes.len(), &adjacent, true);
    for _ in 0..SWEEPS {
        for l in 1..layer_count {
            reorder(&mut layers[l], &left, &pos, &lnodes);
            index_positions(&layers, &mut pos);
        }
        for l in (0..layer_count.saturating_sub(1)).rev() {
            reorder(&mut layers[l], &right, &pos, &lnodes);
            index_positions(&layers, &mut pos);
        }
    }

    // 5. Coordinates. `columns[l]` is the x range of layer `l`.
    // A gap widens to fit the label of an edge that crosses only that gap.
    let mut gaps = vec![LAYER_GAP; layer_count];
    for (edge, chain) in edges.iter().zip(&chains) {
        if let (Some(label), [u, v]) = (&edge.label, chain.as_slice()) {
            let l = lnodes[*u].layer.min(lnodes[*v].layer);
            gaps[l] = gaps[l].max(text_width(label, SMALL_PX) + 24.0);
        }
    }
    let mut columns: Vec<(f64, f64)> = Vec::with_capacity(layer_count);
    let mut x = PAD;
    for (l, layer) in layers.iter().enumerate() {
        let width = layer.iter().map(|&v| lnodes[v].w).fold(0.0, f64::max);
        for &v in layer {
            lnodes[v].x = x + (width - lnodes[v].w) / 2.0;
        }
        let mut y = PAD;
        for &v in layer {
            lnodes[v].y = y;
            y += lnodes[v].h + NODE_GAP;
        }
        columns.push((x, x + width));
        x += width + gaps[l];
    }
    for layer in layers.iter().skip(1) {
        align(layer, &left, &mut lnodes);
    }
    for layer in layers.iter().rev().skip(1) {
        align(layer, &right, &mut lnodes);
    }
    let min_y = lnodes.iter().map(|v| v.y).fold(f64::INFINITY, f64::min);
    if min_y.is_finite() {
        for v in &mut lnodes {
            v.y += PAD - min_y;
        }
    }

    let rects: Vec<Rect> = lnodes
        .iter()
        .map(|v| Rect::new(v.x, v.y, v.w, v.h))
        .collect();

    // 6. Routes. Edges leave a node on its right side and enter on the left
    //    (in layout direction). Through each skipped layer they run
    //    horizontally at their dummy's height, so diagonals stay in the gaps
    //    between layers. Edges sharing a side get spread-out ports.
    let mut middles: Vec<Vec<Point2>> = Vec::with_capacity(edges.len());
    for chain in &chains {
        let mut mid = Vec::new();
        for &d in chain.iter().skip(1).take(chain.len().saturating_sub(2)) {
            let y = lnodes[d].y + lnodes[d].h / 2.0;
            let (x0, x1) = columns[lnodes[d].layer];
            mid.push(Point2::new(x0, y));
            mid.push(Point2::new(x1, y));
        }
        middles.push(mid);
    }
    let mut out_ports: Vec<Vec<(f64, usize)>> = vec![Vec::new(); n];
    let mut in_ports: Vec<Vec<(f64, usize)>> = vec![Vec::new(); n];
    for (e, chain) in chains.iter().enumerate() {
        let (Some(&u), Some(&v)) = (chain.first(), chain.last()) else {
            continue;
        };
        let next_y = middles[e].first().map_or(rects[v].center().y, |p| p.y);
        let prev_y = middles[e].last().map_or(rects[u].center().y, |p| p.y);
        out_ports[u].push((next_y, e));
        in_ports[v].push((prev_y, e));
    }
    let mut start_y = vec![0.0; edges.len()];
    let mut end_y = vec![0.0; edges.len()];
    for (node, rect) in rects.iter().enumerate().take(n) {
        spread(&mut out_ports[node], rect, &mut start_y);
        spread(&mut in_ports[node], rect, &mut end_y);
    }

    let mut scene_edges = Vec::with_capacity(edges.len());
    for (e, (edge, chain)) in edges.iter().zip(&chains).enumerate() {
        let points = match (chain.first(), chain.last()) {
            (Some(&u), Some(&v)) => {
                let mut pts = vec![Point2::new(rects[u].right(), start_y[e])];
                pts.extend(middles[e].iter().copied());
                pts.push(Point2::new(rects[v].x, end_y[e]));
                if oriented[e].is_some_and(|(_, _, reversed)| reversed) {
                    pts.reverse();
                }
                pts
            }
            _ => self_loop(&rects[edge.from]),
        };
        scene_edges.push(SceneEdge {
            from: nodes[edge.from].id.clone(),
            to: nodes[edge.to].id.clone(),
            style: edge.style,
            points,
            label: edge.label.clone(),
        });
    }

    let size = Size2 {
        w: rects.iter().map(Rect::right).fold(0.0, f64::max) + PAD,
        h: rects.iter().map(Rect::bottom).fold(0.0, f64::max) + PAD,
    };
    let scene_nodes = nodes
        .into_iter()
        .zip(&rects)
        .map(|(p, r)| place(p, *r))
        .collect();
    Layout {
        nodes: scene_nodes,
        edges: scene_edges,
        size,
    }
}

/// Reverses DFS back edges, visiting nodes in declaration order.
fn break_cycles(n: usize, oriented: &mut [Option<(usize, usize, bool)>]) {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Active,
        Done,
    }
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (e, o) in oriented.iter().enumerate() {
        if let Some((u, _, _)) = o {
            out[*u].push(e);
        }
    }
    let mut mark = vec![Mark::New; n];
    let mut back = Vec::new();
    for root in 0..n {
        if mark[root] != Mark::New {
            continue;
        }
        // Iterative DFS: (node, next out-edge index).
        let mut stack = vec![(root, 0usize)];
        mark[root] = Mark::Active;
        while let Some(&mut (u, ref mut next)) = stack.last_mut() {
            if let Some(&e) = out[u].get(*next) {
                *next += 1;
                let Some((_, v, _)) = oriented[e] else {
                    continue;
                };
                match mark[v] {
                    Mark::New => {
                        mark[v] = Mark::Active;
                        stack.push((v, 0));
                    }
                    Mark::Active => back.push(e),
                    Mark::Done => {}
                }
            } else {
                mark[u] = Mark::Done;
                stack.pop();
            }
        }
    }
    for e in back {
        if let Some((u, v, r)) = oriented[e] {
            oriented[e] = Some((v, u, !r));
        }
    }
}

fn assign_layers(
    n: usize,
    kinds: &[NodeKind],
    oriented: &[Option<(usize, usize, bool)>],
) -> Vec<usize> {
    let has_sources = kinds.iter().any(|k| k.is_source());
    let base = |k: NodeKind| usize::from(has_sources && !k.is_source());
    let mut indegree = vec![0usize; n];
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (u, v, _) in oriented.iter().flatten() {
        out[*u].push(*v);
        indegree[*v] += 1;
    }
    let mut layer: Vec<usize> = kinds.iter().map(|k| base(*k)).collect();
    // Kahn's algorithm in declaration order.
    let mut ready: Vec<usize> = (0..n).filter(|&v| indegree[v] == 0).rev().collect();
    while let Some(u) = ready.pop() {
        for &v in &out[u] {
            layer[v] = layer[v].max(layer[u] + 1);
            indegree[v] -= 1;
            if indegree[v] == 0 {
                ready.push(v);
            }
        }
    }
    for (l, k) in layer.iter_mut().zip(kinds) {
        if k.is_source() {
            *l = 0;
        }
    }
    let last = kinds
        .iter()
        .zip(&layer)
        .filter(|(k, _)| !k.is_sink())
        .map(|(_, l)| *l + 1)
        .max()
        .unwrap_or(0);
    for (l, k) in layer.iter_mut().zip(kinds) {
        if k.is_sink() {
            *l = last;
        }
    }
    // Close gaps left by pinning so no layer is empty.
    let mut used: Vec<usize> = layer.clone();
    used.sort_unstable();
    used.dedup();
    layer
        .iter()
        .map(|l| used.binary_search(l).unwrap_or(0))
        .collect()
}

fn neighbors(n: usize, adjacent: &[(usize, usize)], forward: bool) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new(); n];
    for &(a, b) in adjacent {
        if forward {
            out[a].push(b);
        } else {
            out[b].push(a);
        }
    }
    out
}

fn reorder(layer: &mut [usize], adj: &[Vec<usize>], pos: &[usize], nodes: &[LNode]) {
    let bary = |v: usize| -> f64 {
        let ns = &adj[v];
        if ns.is_empty() {
            pos[v] as f64
        } else {
            ns.iter().map(|&u| pos[u] as f64).sum::<f64>() / ns.len() as f64
        }
    };
    layer.sort_by(|&a, &b| {
        bary(a)
            .total_cmp(&bary(b))
            .then(nodes[a].key.cmp(&nodes[b].key))
    });
}

/// Moves each node toward the median center of its neighbors, keeping order
/// and `NODE_GAP` spacing.
fn align(layer: &[usize], adj: &[Vec<usize>], nodes: &mut [LNode]) {
    let mut floor = f64::NEG_INFINITY;
    for &v in layer {
        let mut centers: Vec<f64> = adj[v]
            .iter()
            .map(|&u| nodes[u].y + nodes[u].h / 2.0)
            .collect();
        let desired = if centers.is_empty() {
            nodes[v].y
        } else {
            centers.sort_by(f64::total_cmp);
            let mid = centers.len() / 2;
            let median = if centers.len() % 2 == 1 {
                centers[mid]
            } else {
                (centers[mid - 1] + centers[mid]) / 2.0
            };
            median - nodes[v].h / 2.0
        };
        nodes[v].y = desired.max(floor);
        floor = nodes[v].y + nodes[v].h + NODE_GAP;
    }
}

/// Assigns each edge at one side of `rect` a port height, ordered by where
/// the edge goes so that ports don't cross.
fn spread(ports: &mut [(f64, usize)], rect: &Rect, out: &mut [f64]) {
    ports.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let k = ports.len() as f64;
    let step = (rect.h / (k + 1.0)).min(10.0);
    let center = rect.center().y;
    for (i, &(_, e)) in ports.iter().enumerate() {
        out[e] = center + (i as f64 - (k - 1.0) / 2.0) * step;
    }
}

fn self_loop(r: &Rect) -> Vec<Point2> {
    let (right, top) = (r.right(), r.y);
    vec![
        Point2::new(right - 24.0, top),
        Point2::new(right - 24.0, top - 16.0),
        Point2::new(right + 16.0, top - 16.0),
        Point2::new(right + 16.0, top + r.h / 2.0),
        Point2::new(right, top + r.h / 2.0),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::scene::EdgeStyle;

    fn node(id: &str, kind: NodeKind) -> PNode {
        PNode {
            id: id.to_owned(),
            kind,
            label: id.to_owned(),
            sublabel: None,
            source: None,
            badges: Vec::new(),
            fields: Vec::new(),
        }
    }

    fn edge(from: usize, to: usize) -> PEdge {
        PEdge {
            from,
            to,
            style: EdgeStyle::Sync,
            label: None,
        }
    }

    #[test]
    fn layers_and_pins() {
        let kinds = [
            NodeKind::Actor,
            NodeKind::Service,
            NodeKind::Store,
            NodeKind::External,
        ];
        let oriented = [
            Some((0, 1, false)),
            Some((1, 2, false)),
            Some((1, 3, false)),
        ];
        assert_eq!(assign_layers(4, &kinds, &oriented), vec![0, 1, 2, 3]);
    }

    #[test]
    fn cycles_terminate_and_flow_right() {
        let nodes = vec![
            node("A", NodeKind::Service),
            node("B", NodeKind::Service),
            node("C", NodeKind::Service),
        ];
        let edges = [edge(0, 1), edge(1, 2), edge(2, 0), edge(1, 1)];
        let out = layout(nodes, &edges);
        let x = |id: &str| out.nodes.iter().find(|n| n.id == id).unwrap().rect.x;
        assert!(x("A") < x("B") && x("B") < x("C"));
        // The back edge is drawn in its real direction: it starts at C.
        let back = &out.edges[2];
        assert_eq!(back.points[0].x, x("C"));
        assert_eq!(out.edges[3].points.len(), 5);
    }

    #[test]
    fn long_edges_get_bends_and_no_overlap() {
        let nodes = vec![
            node("A", NodeKind::Service),
            node("B", NodeKind::Service),
            node("C", NodeKind::Service),
            node("D", NodeKind::Service),
        ];
        let edges = [edge(0, 1), edge(1, 2), edge(0, 2), edge(0, 3)];
        let out = layout(nodes, &edges);
        assert_eq!(out.edges[2].points.len(), 4);
        for (i, a) in out.nodes.iter().enumerate() {
            for b in &out.nodes[i + 1..] {
                let overlap = a.rect.x < b.rect.right()
                    && b.rect.x < a.rect.right()
                    && a.rect.y < b.rect.bottom()
                    && b.rect.y < a.rect.bottom();
                assert!(!overlap, "{} overlaps {}", a.id, b.id);
            }
        }
    }
}
