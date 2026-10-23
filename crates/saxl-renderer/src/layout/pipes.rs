//! Pipe routing for the tower. Each relation becomes a pipe through the lanes
//! between grid cells; a busy hub's outgoing pipes share a trunk; and a
//! clearance planner keeps unrelated pipes from touching, because touching
//! pipes read as connected.
//!
//! Coordinates are grid space: `x` along columns, `y` along rows, `z` up
//! from the lowest tier's base. A cell's center is `(c × CX, r × CY)`. Every
//! container owns a cell no other tier uses, and lanes (the gaps between
//! rows and columns) are empty on every tier, so a pipe changes tier only in
//! a lane.

/// Column pitch.
pub(crate) const CX: f64 = 4.4;
/// Row pitch.
pub(crate) const CY: f64 = 3.6;
/// Height between tiers.
pub(crate) const TIER_H: f64 = 4.4;
/// Radius of a relation's pipe.
pub(crate) const PIPE_R: f64 = 0.1;
/// Radius of a hub's trunk.
pub(crate) const TRUNK_R: f64 = 0.15;
/// How far in front of an actor's center its terminal's face is.
pub(crate) const ACTOR_FACE: f64 = 0.3;
/// Minimum air between two unrelated pipes.
const GAP: f64 = 0.1;
/// A container with this many outgoing relations gets a trunk.
const HUB_MIN: usize = 4;
/// Sideways offsets: a pipe's track in a lane, and its offset along a face.
const TRACKS: [f64; 9] = [0.0, -0.32, 0.32, -0.64, 0.64, -0.16, 0.16, -0.48, 0.48];
/// Run heights above a tier. Above face height a pipe climbs in the lane.
const HEIGHTS: [f64; 7] = [0.3, 0.5, 0.7, 0.9, 1.3, 1.7, 2.1];
/// Trunk heights, overhead first so face-level pipes pass underneath.
const TRUNK_HEIGHTS: [f64; 8] = [1.3, 1.7, 2.1, 0.6, 0.4, 0.8, 0.3, 0.9];
/// Orderings tried when the first leaves a clash.
const RETRIES: u64 = 12;

/// A point in grid space.
pub(crate) type P = [f64; 3];

/// What occupies a cell, which decides where its faces are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    Service,
    Store,
    Queue,
    Actor,
    External,
}

/// An element's cell: column, row, and tier.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cell {
    pub c: i32,
    pub r: i32,
    pub t: i32,
    pub shape: Shape,
}

/// A relation to route, by index into the cells.
#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub id: String,
    pub from: usize,
    pub to: usize,
    pub is_async: bool,
}

/// A routed relation.
#[derive(Debug, Clone)]
pub(crate) struct Routed {
    pub id: String,
    pub from: usize,
    pub to: usize,
    pub is_async: bool,
    /// The tube; a branch starts at its tee on the trunk.
    pub points: Vec<P>,
    /// From `from` to `to`, through the trunk for a branch.
    pub route: Vec<P>,
    /// The hub (cell index) whose trunk this branches from.
    pub trunk: Option<usize>,
}

/// A hub's shared trunk.
#[derive(Debug, Clone)]
pub(crate) struct RoutedTrunk {
    pub hub: usize,
    pub stub: Vec<P>,
    pub run: Vec<P>,
    pub pipes: Vec<String>,
}

/// Every pipe and trunk, and how many pipes still touch an unrelated one.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub pipes: Vec<Routed>,
    pub trunks: Vec<RoutedTrunk>,
    pub clashes: u32,
}

/// Routes every edge. Tries the hardest-first ordering, then a few
/// deterministic reorderings while any clash remains, and keeps the best.
pub(crate) fn plan(cells: &[Cell], edges: &[Edge]) -> Plan {
    let mut best = plan_once(cells, edges, 0);
    for seed in 1..=RETRIES {
        if best.clashes == 0 {
            break;
        }
        let p = plan_once(cells, edges, seed);
        if p.clashes < best.clashes {
            best = p;
        }
    }
    best
}

// ---------- Geometry ----------

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn along(a: P, d: P, s: f64) -> P {
    [a[0] + d[0] * s, a[1] + d[1] * s, a[2] + d[2] * s]
}

fn dist(a: P, b: P) -> f64 {
    dot(sub(a, b), sub(a, b)).sqrt()
}

fn unit(d: P) -> P {
    let l = dot(d, d).sqrt();
    if l < 1e-12 {
        d
    } else {
        [d[0] / l, d[1] / l, d[2] / l]
    }
}

/// Closest distance between segments `p1q1` and `p2q2` (Ericson,
/// Real-Time Collision Detection, 5.1.9).
fn seg_dist(p1: P, q1: P, p2: P, q2: P) -> f64 {
    let (d1, d2, r) = (sub(q1, p1), sub(q2, p2), sub(p1, p2));
    let (a, e, f) = (dot(d1, d1), dot(d2, d2), dot(d2, r));
    if a < 1e-9 && e < 1e-9 {
        return dist(p1, p2);
    }
    let (s, t) = if a < 1e-9 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = dot(d1, r);
        if e < 1e-9 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = dot(d1, d2);
            let den = a * e - b * b;
            let mut s = if den > 1e-12 {
                ((b * f - c * e) / den).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    dist(along(p1, d1, s), along(p2, d2, t))
}

/// Drops repeated points and the middle points of straight runs.
fn simplify(points: Vec<P>) -> Vec<P> {
    let mut out: Vec<P> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_none_or(|q| dist(*q, p) > 1e-4) {
            out.push(p);
        }
    }
    let mut i = out.len().saturating_sub(2);
    while i > 0 {
        let a = unit(sub(out[i], out[i - 1]));
        let b = unit(sub(out[i + 1], out[i]));
        if dot(a, b) > 0.9999 {
            out.remove(i);
        }
        i -= 1;
    }
    out
}

// ---------- Faces ----------

/// Distance from a cell's center to its front or back face, at sideways
/// offset `s`.
fn depth_y(shape: Shape, s: f64) -> f64 {
    match shape {
        Shape::Service => 0.8,
        Shape::Store => (0.81 - s * s).max(0.01).sqrt(),
        Shape::Queue => 0.45,
        Shape::Actor => ACTOR_FACE,
        Shape::External => 0.6,
    }
}

/// Distance from a cell's center to its left or right face, at offset `s`.
fn depth_x(shape: Shape, s: f64) -> f64 {
    match shape {
        Shape::Service => 1.4,
        Shape::Store => (0.81 - s * s).max(0.01).sqrt(),
        Shape::Queue => 1.8,
        Shape::Actor => ACTOR_FACE,
        Shape::External => 1.0,
    }
}

/// How far along a face a pipe may attach.
fn half_width(shape: Shape) -> f64 {
    match shape {
        Shape::Service => 1.2,
        Shape::Store => 0.6,
        Shape::Queue => 1.4,
        Shape::Actor => 0.15,
        Shape::External => 0.8,
    }
}

/// The height a pipe attaches to a face at: its run height when that is on
/// the face, otherwise mid-face (the pipe then climbs in the lane).
fn face_h(h: f64, shape: Shape) -> f64 {
    let top = if shape == Shape::Actor { 0.75 } else { 0.9 };
    if h <= top { h } else { 0.6 }
}

fn clamp(x: f64, a: f64) -> f64 {
    x.clamp(-a, a)
}

// ---------- Routes ----------

struct Grid<'a> {
    cells: &'a [Cell],
}

impl Grid<'_> {
    fn free(&self, c: i32, r: i32, t: i32) -> bool {
        !self.cells.iter().any(|x| x.c == c && x.r == r && x.t == t)
    }

    /// One pipe from `a` to `b` with lane track `t`, run height `h`, and face
    /// offset `e`. Neighbors on a free straight line connect directly.
    /// Otherwise the pipe leaves through the face toward `b`, runs along the
    /// lane between rows, turns into the lane between columns beside `b`,
    /// changes tier there, and enters `b`'s facing side. Actors always leave
    /// through their terminal's face (+x).
    fn route(&self, a: &Cell, b: &Cell, t: f64, h: f64, e: f64) -> Vec<P> {
        let (ax, ay, za) = (
            f64::from(a.c) * CX,
            f64::from(a.r) * CY,
            f64::from(a.t) * TIER_H,
        );
        let (bx, by, zb) = (
            f64::from(b.c) * CX,
            f64::from(b.r) * CY,
            f64::from(b.t) * TIER_H,
        );
        let (ha, hb) = (face_h(h, a.shape), face_h(h, b.shape));
        let down = match b.r.cmp(&a.r) {
            std::cmp::Ordering::Less => -1.0,
            _ => 1.0,
        };
        let same_row = a.t == b.t && a.r == b.r;
        if a.shape == Shape::Actor && !same_row {
            let s = clamp(e * 0.5, 0.25);
            let col = (f64::from(a.c) + 0.5) * CX + t;
            let lane_b = (f64::from(b.r) - 0.5 * down) * CY + t;
            let sb = clamp(e, half_width(b.shape));
            return vec![
                [ax + depth_x(a.shape, s), ay + s, za + ha],
                [col, ay + s, za + ha],
                [col, ay + s, zb + h],
                [col, lane_b, zb + h],
                [bx + sb, lane_b, zb + h],
                [bx + sb, lane_b, zb + hb],
                [bx + sb, by - down * depth_y(b.shape, sb), zb + hb],
            ];
        }
        if same_row && (a.c.min(b.c) + 1..a.c.max(b.c)).all(|c| self.free(c, a.r, a.t)) {
            let lim = if a.shape == Shape::Actor {
                0.25
            } else {
                half_width(a.shape) * 0.5
            };
            let s = clamp(e * 0.5, lim);
            let dir = f64::from((b.c - a.c).signum());
            return vec![
                [ax + dir * depth_x(a.shape, s), ay + s, za + ha],
                [bx - dir * depth_x(b.shape, s), ay + s, za + ha],
            ];
        }
        if a.t == b.t
            && a.c == b.c
            && (a.r.min(b.r) + 1..a.r.max(b.r)).all(|r| self.free(a.c, r, a.t))
        {
            let s = clamp(e * 0.5, half_width(a.shape) * 0.5);
            let dir = f64::from((b.r - a.r).signum());
            return vec![
                [ax + s, ay + dir * depth_y(a.shape, s), za + ha],
                [ax + s, by - dir * depth_y(b.shape, s), za + ha],
            ];
        }
        let lane_a = (f64::from(a.r) + 0.5 * down) * CY + t;
        let lane_b = (f64::from(b.r) - 0.5 * if b.r == a.r { -1.0 } else { down }) * CY + t;
        let col = if b.c > a.c {
            f64::from(b.c) - 0.5
        } else {
            f64::from(b.c) + 0.5
        } * CX
            + t;
        let (sa, sb) = (clamp(e, half_width(a.shape)), clamp(e, half_width(b.shape)));
        let (face_a, face_b) = ((lane_a - ay).signum(), (lane_b - by).signum());
        vec![
            [ax + sa, ay + face_a * depth_y(a.shape, sa), za + ha],
            [ax + sa, lane_a, za + ha],
            [ax + sa, lane_a, za + h],
            [col, lane_a, za + h],
            [col, lane_a, zb + h],
            [col, lane_b, zb + h],
            [bx + sb, lane_b, zb + h],
            [bx + sb, lane_b, zb + hb],
            [bx + sb, by + face_b * depth_y(b.shape, sb), zb + hb],
        ]
    }
}

// ---------- Clearance ----------

/// A placed pipe's segments, with a bounding box for a quick reject.
struct Placed {
    owner: String,
    segs: Vec<(P, P, f64)>,
    lo: P,
    hi: P,
}

impl Placed {
    fn new(owner: String, segs: Vec<(P, P, f64)>) -> Self {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for (a, b, r) in &segs {
            for p in [a, b] {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k] - r);
                    hi[k] = hi[k].max(p[k] + r);
                }
            }
        }
        Self {
            owner,
            segs,
            lo,
            hi,
        }
    }
}

fn segs_of(points: &[P], r: f64) -> Vec<(P, P, f64)> {
    points.windows(2).map(|w| (w[0], w[1], r)).collect()
}

/// The owners of placed pipes that `candidate` would touch, except `skip`.
fn clashes(placed: &[&Placed], candidate: &Placed, skip: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for p in placed {
        if skip.contains(&p.owner.as_str()) {
            continue;
        }
        let apart =
            (0..3).any(|k| candidate.lo[k] > p.hi[k] + GAP || p.lo[k] > candidate.hi[k] + GAP);
        if apart {
            continue;
        }
        let touch = candidate.segs.iter().any(|&(a, b, r)| {
            p.segs
                .iter()
                .any(|&(c, d, r2)| seg_dist(a, b, c, d) < r + r2 + GAP)
        });
        if touch {
            hits.push(p.owner.clone());
        }
    }
    hits
}

/// Tries every (track, height, face offset); keeps the first placement that
/// touches nothing, else the one touching fewest.
fn choose(
    placed: &[&Placed],
    owner: &str,
    skip: &[&str],
    heights: &[f64],
    make: impl Fn(f64, f64, f64) -> Vec<P>,
) -> (Vec<P>, u32) {
    let mut best: Option<(Vec<P>, u32)> = None;
    for &t in &TRACKS {
        for &h in heights {
            for &e in &TRACKS {
                let points = make(t, h, e);
                let candidate = Placed::new(owner.to_owned(), segs_of(&points, PIPE_R));
                let n = u32::try_from(clashes(placed, &candidate, skip).len()).unwrap_or(u32::MAX);
                if best.as_ref().is_none_or(|(_, m)| n < *m) {
                    best = Some((points, n));
                }
                if n == 0 {
                    return best.unwrap_or_default();
                }
            }
        }
    }
    best.unwrap_or_default()
}

// ---------- Planning ----------

/// A hub's trunk, branches, and the clashes they cause.
struct HubPlan {
    pipes: Vec<Routed>,
    trunk: RoutedTrunk,
    entries: Vec<Placed>,
    clashes: u32,
}

/// Places a hub's branches, then the trunk over every tee, at trunk height
/// `th`. The trunk leaves the hub into the lane on the side where most
/// targets are and runs along it; each branch changes tier at its tee (lanes
/// are empty on every tier) and continues to its target. Targets whose row
/// borders the lane branch straight in; the rest turn into the column lane
/// beside them first.
fn place_hub(cells: &[Cell], edges: &[&Edge], hub: usize, th: f64, placed: &[Placed]) -> HubPlan {
    let hc = cells[hub];
    let (hx, hy, zh) = (
        f64::from(hc.c) * CX,
        f64::from(hc.r) * CY,
        f64::from(hc.t) * TIER_H,
    );
    let up = edges.iter().filter(|e| cells[e.to].r < hc.r).count();
    let below = edges.iter().filter(|e| cells[e.to].r > hc.r).count();
    let side = if below > up { 1.0 } else { -1.0 };
    let lane_row = f64::from(hc.r) + 0.5 * side;
    let lane_y = lane_row * CY;
    let owner = format!("trunk:{hub}");
    let hf = face_h(th, hc.shape);
    let start = [
        [hx, hy + side * depth_y(hc.shape, 0.0), zh + hf],
        [hx, lane_y, zh + hf],
        [hx, lane_y, zh + th],
    ];
    let mut local: Vec<Placed> = Vec::new();
    let (mut pipes, mut tees, mut total) = (Vec::new(), Vec::new(), 0);
    for e in edges {
        let b = cells[e.to];
        let (bx, by, zb) = (
            f64::from(b.c) * CX,
            f64::from(b.r) * CY,
            f64::from(b.t) * TIER_H,
        );
        let make = |t: f64, h: f64, off: f64| -> Vec<P> {
            let hb = face_h(h, b.shape);
            let (tee_x, rest) = if (f64::from(b.r) - lane_row).abs() == 0.5 {
                let sb = clamp(off * 1.4, half_width(b.shape));
                let face = (lane_y - by).signum();
                (
                    bx + sb,
                    vec![
                        [bx + sb, lane_y, zb + hb],
                        [bx + sb, by + face * depth_y(b.shape, sb), zb + hb],
                    ],
                )
            } else {
                let sb = clamp(off, half_width(b.shape));
                let tee_x = if b.c > hc.c {
                    f64::from(b.c) - 0.5
                } else {
                    f64::from(b.c) + 0.5
                } * CX
                    + t;
                let dir = if f64::from(b.r) > lane_row { 1.0 } else { -1.0 };
                let lane_b = (f64::from(b.r) - 0.5 * dir) * CY + t;
                (
                    tee_x,
                    vec![
                        [tee_x, lane_y, zb + h],
                        [tee_x, lane_b, zb + h],
                        [bx + sb, lane_b, zb + h],
                        [bx + sb, lane_b, zb + hb],
                        [bx + sb, by - dir * depth_y(b.shape, sb), zb + hb],
                    ],
                )
            };
            let mut points = vec![[tee_x, lane_y, zh + th]];
            points.extend(rest);
            simplify(points)
        };
        let seen: Vec<&Placed> = placed.iter().chain(local.iter()).collect();
        let (points, n) = choose(&seen, &e.id, &[owner.as_str()], &HEIGHTS, make);
        total += n;
        tees.push(points.first().map_or(hx, |p| p[0]));
        let mut route = start.to_vec();
        route.extend(points.iter().copied());
        local.push(Placed::new(e.id.clone(), segs_of(&points, PIPE_R)));
        pipes.push(Routed {
            id: e.id.clone(),
            from: e.from,
            to: e.to,
            is_async: e.is_async,
            points,
            route: simplify(route),
            trunk: Some(hub),
        });
    }
    let x0 = tees.iter().copied().fold(hx, f64::min) - 0.25;
    let x1 = tees.iter().copied().fold(hx, f64::max) + 0.25;
    let stub = simplify(start.to_vec());
    let run = vec![[x0, lane_y, zh + th], [x1, lane_y, zh + th]];
    let mut segs = segs_of(&stub, TRUNK_R);
    segs.extend(segs_of(&run, TRUNK_R));
    let trunk_placed = Placed::new(owner, segs);
    let branch_ids: Vec<&str> = edges.iter().map(|e| e.id.as_str()).collect();
    let seen: Vec<&Placed> = placed.iter().collect();
    total += u32::try_from(clashes(&seen, &trunk_placed, &branch_ids).len()).unwrap_or(u32::MAX);
    local.push(trunk_placed);
    HubPlan {
        trunk: RoutedTrunk {
            hub,
            stub,
            run,
            pipes: edges.iter().map(|e| e.id.clone()).collect(),
        },
        pipes,
        entries: local,
        clashes: total,
    }
}

fn plan_once(cells: &[Cell], edges: &[Edge], seed: u64) -> Plan {
    let grid = Grid { cells };
    let mut placed: Vec<Placed> = Vec::new();
    let mut out = Plan {
        pipes: Vec::new(),
        trunks: Vec::new(),
        clashes: 0,
    };

    // Hubs: containers (not actors or externals) with many outgoing relations.
    let mut hubs: Vec<usize> = Vec::new();
    for (i, cell) in cells.iter().enumerate() {
        let internal = !matches!(cell.shape, Shape::Actor | Shape::External);
        if internal && edges.iter().filter(|e| e.from == i).count() >= HUB_MIN {
            hubs.push(i);
        }
    }
    // Hubs fed by other hubs go first, so the feeding hub's branches see (and
    // avoid) the fed hub's trunk.
    let fed_by: Vec<usize> = hubs
        .iter()
        .map(|&h| {
            edges
                .iter()
                .filter(|e| e.to == h && hubs.contains(&e.from))
                .count()
        })
        .collect();
    let mut by_feed: Vec<(usize, usize)> = hubs.iter().copied().zip(fed_by).collect();
    by_feed.sort_by_key(|&(_, f)| std::cmp::Reverse(f));
    let hubs: Vec<usize> = by_feed.into_iter().map(|(h, _)| h).collect();
    for &hub in &hubs {
        let list: Vec<&Edge> = edges.iter().filter(|e| e.from == hub).collect();
        let mut best: Option<HubPlan> = None;
        for &th in &TRUNK_HEIGHTS {
            let p = place_hub(cells, &list, hub, th, &placed);
            let n = p.clashes;
            if best.as_ref().is_none_or(|b| n < b.clashes) {
                best = Some(p);
            }
            if n == 0 {
                break;
            }
        }
        if let Some(b) = best {
            out.clashes += b.clashes;
            placed.extend(b.entries);
            out.pipes.extend(b.pipes);
            out.trunks.push(b.trunk);
        }
    }

    // Single pipes, hardest first: more tiers and cells to cross pick before
    // short hops. A seed shuffles the order deterministically for a retry.
    let span = |e: &Edge| {
        let (a, b) = (cells[e.from], cells[e.to]);
        (a.t - b.t).abs() * 3 + (a.c - b.c).abs() + (a.r - b.r).abs()
    };
    let mut order: Vec<&Edge> = edges.iter().filter(|e| !hubs.contains(&e.from)).collect();
    order.sort_by_key(|e| std::cmp::Reverse(span(e)));
    let mut x = seed;
    for k in (1..order.len()).rev() {
        if seed == 0 {
            break;
        }
        x = (x.wrapping_mul(1_103_515_245).wrapping_add(12_345)) % 2_147_483_648;
        let j = usize::try_from(x).unwrap_or(0) % (k + 1);
        order.swap(k, j);
    }
    for e in order {
        let (a, b) = (cells[e.from], cells[e.to]);
        let make = |t: f64, h: f64, off: f64| simplify(grid.route(&a, &b, t, h, off));
        let seen: Vec<&Placed> = placed.iter().collect();
        let (points, n) = choose(&seen, &e.id, &[], &HEIGHTS, make);
        out.clashes += n;
        placed.push(Placed::new(e.id.clone(), segs_of(&points, PIPE_R)));
        out.pipes.push(Routed {
            id: e.id.clone(),
            from: e.from,
            to: e.to,
            is_async: e.is_async,
            route: points.clone(),
            points,
            trunk: None,
        });
    }
    // Keep the relations' declaration order in the output.
    out.pipes
        .sort_by_key(|p| edges.iter().position(|e| e.id == p.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossing_segments_touch_and_parallel_ones_clear() {
        let d = seg_dist(
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
        );
        assert!(d.abs() < 1e-9);
        let d = seg_dist(
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [2.0, 0.5, 0.0],
        );
        assert!((d - 0.5).abs() < 1e-9);
    }

    #[test]
    fn simplify_drops_repeats_and_straight_middles() {
        let s = simplify(vec![
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
        ]);
        assert_eq!(s, vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0]]);
    }
}
