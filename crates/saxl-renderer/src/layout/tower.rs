//! Tower layout (3D, world units, y up): the deployment floor, containers
//! suspended above it in tiers, and relations drawn as pipes.
//!
//! Each deployment group (a cluster or a managed block) is a tier, stacked in
//! declaration order from the top down. Tiers take side-by-side bands of grid
//! columns, so no two containers share a cell (column and row) and every
//! leader from a replica up to its container drops through empty space.
//! Actors stand in a column left of the grid on the top tier; externals in a
//! column right of it, at the tier of a container they relate to. Pipes are
//! routed through the lanes between cells by [`pipes`].

use saxl_core::{ElementKind, GroupKind as IrGroupKind};
use saxl_validator::ValidatedModel;

use crate::layout::node_size;
use crate::layout::pipes::{self, CX, CY, Cell, P, PIPE_R, Shape, TIER_H, TRUNK_R};
use crate::project::{self, PNode};
use crate::scene::{
    Boundary, Box3, Label3, Leader, NodeKind, Pipe, Pod, Point3, Tag, Tier, Tower, Trunk,
};

/// Pixels per world unit, for sizing labels' boxes from the 2D metric.
const SCALE: f64 = 60.0;
/// Height of the lowest tier's base above the floor.
const LEVEL: f64 = 7.0;
/// Service box: width, height, depth.
const SERVICE: (f64, f64, f64) = (2.8, 1.0, 1.6);
/// Store cylinder: radius and height.
const STORE: (f64, f64) = (0.9, 1.2);
/// Queue pipe along x: length, radius, and axis height above the tier.
const QUEUE: (f64, f64, f64) = (3.6, 0.45, 0.62);
/// An actor's figure and terminal fit in this box.
const ACTOR: (f64, f64, f64) = (1.5, 1.45, 1.5);
/// External (ghost) box.
const EXTERNAL: (f64, f64, f64) = (2.0, 1.0, 1.2);
/// Replica cube and managed pad.
const POD: f64 = 0.7;
const PAD: (f64, f64) = (0.95, 0.12);
/// How far below the lowest tier the boundary starts, and above the top
/// tier's base it ends.
const BOUNDARY: (f64, f64) = (0.4, 1.6);

struct TierInfo {
    label: String,
    members: Vec<usize>,
}

/// Library kinds reuse the closest system shape until the tower draws them.
fn shape(kind: ElementKind) -> Shape {
    match kind {
        ElementKind::Actor | ElementKind::Consumer => Shape::Actor,
        ElementKind::External | ElementKind::Package | ElementKind::Hook => Shape::External,
        ElementKind::Service | ElementKind::Api | ElementKind::Module => Shape::Service,
        ElementKind::Store => Shape::Store,
        ElementKind::Queue => Shape::Queue,
    }
}

/// An element's box: its kind's shape, `base` the tier's height.
fn element_box(n: &PNode, id: &str, kind: ElementKind, x: f64, z: f64, base: f64) -> Box3 {
    let (size, mid) = match kind {
        ElementKind::Service | ElementKind::Api | ElementKind::Module => (
            Point3::new(SERVICE.0, SERVICE.1, SERVICE.2),
            SERVICE.1 / 2.0,
        ),
        ElementKind::Store => (
            Point3::new(2.0 * STORE.0, STORE.1, 2.0 * STORE.0),
            STORE.1 / 2.0,
        ),
        ElementKind::Queue => (Point3::new(QUEUE.0, 2.0 * QUEUE.1, 2.0 * QUEUE.1), QUEUE.2),
        ElementKind::Actor | ElementKind::Consumer => {
            (Point3::new(ACTOR.0, ACTOR.1, ACTOR.2), ACTOR.1 / 2.0)
        }
        ElementKind::External | ElementKind::Package | ElementKind::Hook => (
            Point3::new(EXTERNAL.0, EXTERNAL.1, EXTERNAL.2),
            EXTERNAL.1 / 2.0,
        ),
    };
    Box3 {
        id: id.to_owned(),
        kind: kind.into(),
        center: Point3::new(x, base + mid, z),
        size,
        label: Some(n.label.clone()),
        sublabel: n.sublabel.clone(),
    }
}

/// How far from its center along x a leader may meet a container's underside.
fn element_reach(kind: ElementKind) -> f64 {
    match kind {
        ElementKind::Store => STORE.0 * 0.8,
        ElementKind::Queue => QUEUE.0 / 2.0 - 0.2,
        _ => SERVICE.0 / 2.0 - 0.2,
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

pub(crate) fn layout(vm: &ValidatedModel) -> Tower {
    let model = vm.model();
    let resolved = vm.resolved();
    let n = model.elements.len();

    // Tiers: one per deployment group, keyed by its caption, in declaration
    // order; an element joins the group of its first deployment.
    let group_label = |r: usize, g: usize| {
        let group = &model.regions[r].groups[g];
        match group.kind {
            IrGroupKind::Cluster => format!(
                "{} cluster",
                group.name.as_ref().map_or("unnamed", |n| n.text.as_str())
            ),
            IrGroupKind::Managed => "managed".to_owned(),
        }
    };
    let mut tiers: Vec<TierInfo> = Vec::new();
    for (r, region) in model.regions.iter().enumerate() {
        for g in 0..region.groups.len() {
            let label = group_label(r, g);
            if !tiers.iter().any(|t| t.label == label) {
                tiers.push(TierInfo {
                    label,
                    members: Vec::new(),
                });
            }
        }
    }
    let mut undeployed = Vec::new();
    for (i, e) in model.elements.iter().enumerate() {
        if !e.kind.is_internal() {
            continue;
        }
        match resolved.deployments[i].first() {
            Some(d) => {
                let label = group_label(d.region, d.group);
                if let Some(t) = tiers.iter_mut().find(|t| t.label == label) {
                    t.members.push(i);
                }
            }
            None => undeployed.push(i),
        }
    }
    if !undeployed.is_empty() {
        tiers.insert(
            0,
            TierInfo {
                label: "not deployed".to_owned(),
                members: undeployed,
            },
        );
    }
    tiers.retain(|t| !t.members.is_empty());
    let top_tier = tiers.len().saturating_sub(1) as i32;

    // Which elements each element relates to, either direction.
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (a, b) in resolved.relation_ends.iter().flatten() {
        neighbors[a.index()].push(b.index());
        neighbors[b.index()].push(a.index());
    }

    // Grid: each tier fills its own band of columns, column by column. Tiers
    // after the first order their members by where their neighbors already
    // sit, so related containers share rows.
    let internal: usize = tiers.iter().map(|t| t.members.len()).sum();
    let rows = ((internal as f64 / 1.5).sqrt().ceil() as i32).max(2);
    let mut cell: Vec<Option<(i32, i32, i32)>> = vec![None; n];
    let mut bands: Vec<(String, i32, i32, i32)> = Vec::new();
    let mut col0 = 0;
    for (k, tier) in tiers.iter().enumerate() {
        let t = top_tier - k as i32;
        let cols = ((tier.members.len() as f64 / f64::from(rows)).ceil() as i32).max(1);
        let mut order: Vec<(f64, usize)> = tier
            .members
            .iter()
            .map(|&i| {
                let placed: Vec<f64> = neighbors[i]
                    .iter()
                    .filter_map(|&j| cell[j].map(|c| f64::from(c.1)))
                    .collect();
                (mean(&placed).unwrap_or(f64::INFINITY), i)
            })
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (slot, &(_, i)) in order.iter().enumerate() {
            let slot = slot as i32;
            cell[i] = Some((col0 + slot / rows, slot % rows, t));
        }
        bands.push((tier.label.clone(), col0, col0 + cols - 1, t));
        col0 += cols;
    }
    let cols = col0;

    // Outside: actors left of the grid on the top tier, externals right of it
    // at a related container's tier; each in the free row nearest its
    // neighbors' rows.
    for (kind, col) in [(ElementKind::Actor, -1), (ElementKind::External, cols)] {
        let mut used: Vec<i32> = Vec::new();
        for (i, e) in model.elements.iter().enumerate() {
            if e.kind != kind {
                continue;
            }
            let placed: Vec<(i32, i32)> = neighbors[i]
                .iter()
                .filter_map(|&j| cell[j].map(|c| (c.1, c.2)))
                .collect();
            let rows_of: Vec<f64> = placed.iter().map(|p| f64::from(p.0)).collect();
            let want = mean(&rows_of).map_or(0, |m| m.round() as i32);
            let tier = if kind == ElementKind::Actor {
                top_tier
            } else {
                placed.first().map_or(top_tier, |p| p.1)
            };
            let row = (0..)
                .flat_map(|d| [want + d, want - d])
                .find(|r| !used.contains(r))
                .unwrap_or(want);
            used.push(row);
            cell[i] = Some((col, row, tier));
        }
    }

    // Grid to world: centered on the origin, `LEVEL` up from the floor.
    let x0 = f64::from(cols - 1) * CX / 2.0;
    let y0 = f64::from(rows - 1) * CY / 2.0;
    let world = |p: P| Point3::new(p[0] - x0, LEVEL + p[2], p[1] - y0);
    let at = |i: usize| {
        cell[i].map(|(c, r, t)| {
            (
                f64::from(c) * CX - x0,
                f64::from(r) * CY - y0,
                f64::from(t) * TIER_H,
            )
        })
    };

    let mut tower = Tower {
        slabs: Vec::new(),
        labels: Vec::new(),
        pods: Vec::new(),
        tags: Vec::new(),
        infra: Vec::new(),
        level: LEVEL,
        tiers: Vec::new(),
        containers: Vec::new(),
        outside: Vec::new(),
        leaders: Vec::new(),
        pipes: Vec::new(),
        trunks: Vec::new(),
        boundary: Boundary {
            label: project::system_name(model).to_owned(),
            min: world([-0.5 * CX, -0.5 * CY, -BOUNDARY.0]),
            max: world([
                (f64::from(cols) - 0.5) * CX,
                (f64::from(rows) - 0.5) * CY,
                f64::from(top_tier) * TIER_H + BOUNDARY.1,
            ]),
        },
        clashes: 0,
    };

    // Containers, actors, and externals.
    for (i, e) in model.elements.iter().enumerate() {
        let Some((x, z, base)) = at(i) else { continue };
        let b = element_box(
            &project::element_node(model, i),
            &e.name.text,
            e.kind,
            x,
            z,
            LEVEL + base,
        );
        if e.kind.is_internal() {
            tower.containers.push(b);
        } else {
            tower.outside.push(b);
        }
    }

    // Tier outlines: just under each band's cells.
    for (label, c0, c1, t) in &bands {
        let inset = 0.15;
        let h = f64::from(*t) * TIER_H - 0.05;
        tower.tiers.push(Tier {
            id: label.clone(),
            label: label.clone(),
            min: world([(f64::from(*c0) - 0.5) * CX + inset, -0.5 * CY + inset, h]),
            max: world([
                (f64::from(*c1) + 0.5) * CX - inset,
                (f64::from(rows) - 0.5) * CY - inset,
                h,
            ]),
        });
    }

    // Floor: one plate per region, side by side along x; in each, one zone
    // per group under its tier's band.
    let plate_w = (f64::from(cols) + 0.8) * CX;
    let (fz0, fz1) = (-0.9 * CY - y0, (f64::from(rows) - 0.1) * CY - y0);
    let deploy = project::deployment(vm);
    for (r, region) in model.regions.iter().enumerate() {
        let shift = r as f64 * (plate_w + 2.0);
        let (fx0, fx1) = (-0.9 * CX - x0 + shift, -0.9 * CX - x0 + shift + plate_w);
        tower.slabs.push(Box3 {
            id: format!("region:{}", region.name.text),
            kind: NodeKind::Floor,
            center: Point3::new((fx0 + fx1) / 2.0, -0.25, (fz0 + fz1) / 2.0),
            size: Point3::new(fx1 - fx0, 0.5, fz1 - fz0),
            label: Some(region.name.text.clone()),
            sublabel: Some("region".to_owned()),
        });
        tower.labels.push(Label3 {
            text: format!("{} · REGION", region.name.text.to_uppercase()),
            position: Point3::new(fx0 + 0.5, 0.0, fz1 - 0.25),
        });
        for g in 0..region.groups.len() {
            let label = group_label(r, g);
            let Some((_, c0, c1, _)) = bands.iter().find(|b| b.0 == label) else {
                continue;
            };
            let zx0 = (f64::from(*c0) - 0.5) * CX - x0 + shift + 0.1;
            let zx1 = (f64::from(*c1) + 0.5) * CX - x0 + shift - 0.1;
            let (zz0, zz1) = (-0.5 * CY - y0, (f64::from(rows) - 0.5) * CY - y0);
            tower.slabs.push(Box3 {
                id: format!("group:{}/{label}", region.name.text),
                kind: NodeKind::Floor,
                center: Point3::new((zx0 + zx1) / 2.0, 0.11, (zz0 + zz1) / 2.0),
                size: Point3::new(zx1 - zx0, 0.22, zz1 - zz0),
                label: Some(label.clone()),
                sublabel: Some("group".to_owned()),
            });
            tower.labels.push(Label3 {
                text: label.to_uppercase(),
                position: Point3::new(zx0 + 0.4, 0.0, zz1 - 0.5),
            });
        }
        // Infrastructure nodes: on the floor, left of the region's plate.
        if let Some(d) = deploy.get(r) {
            for (k, p) in d.infra.iter().enumerate() {
                let w = node_size(p).0 / SCALE;
                tower.infra.push(Box3 {
                    id: p.label.clone(),
                    kind: NodeKind::Infra,
                    center: Point3::new(fx0 - 1.0 - w / 2.0, 0.14, fz0 + 1.2 + 1.6 * k as f64),
                    size: Point3::new(w, 0.28, 1.2),
                    label: Some(p.label.clone()),
                    sublabel: p.sublabel.clone(),
                });
            }
        }
    }

    // Replicas, their tags, and leaders up to the containers.
    for (i, e) in model.elements.iter().enumerate() {
        let Some((x, z, base)) = at(i) else { continue };
        let bottom = LEVEL
            + base
            + if e.kind == ElementKind::Queue {
                QUEUE.2 - QUEUE.1
            } else {
                0.0
            };
        for d in &resolved.deployments[i] {
            let managed = model.regions[d.region].groups[d.group].kind == IrGroupKind::Managed;
            let count = if d.primary_replica {
                2
            } else {
                d.replicas.max(1)
            };
            let shift = d.region as f64 * (plate_w + 2.0);
            let pitch: f64 = if managed { 1.15 } else { 0.95 };
            let spacing = if count > 1 {
                pitch.min(3.4 / f64::from(count - 1))
            } else {
                0.0
            };
            for k in 0..count {
                let px = x + shift + (f64::from(k) - f64::from(count - 1) / 2.0) * spacing;
                let (size, h) = if managed {
                    (Point3::new(PAD.0, PAD.1, PAD.0), PAD.1)
                } else {
                    (Point3::new(POD, POD, POD), POD)
                };
                tower.pods.push(Pod {
                    element: e.name.text.clone(),
                    managed,
                    geometry: Box3 {
                        id: format!(
                            "pod:{}/{}#{k}",
                            model.regions[d.region].name.text, e.name.text
                        ),
                        kind: e.kind.into(),
                        center: Point3::new(px, h / 2.0, z),
                        size,
                        label: None,
                        sublabel: None,
                    },
                });
                // A leader rises straight when its pod is under the container;
                // pods spread wider (or on another region's plate) slant in to
                // meet the container's underside.
                let reach = element_reach(e.kind);
                let top = if d.region == 0 {
                    Point3::new(x + (px - x).clamp(-reach, reach), bottom, z)
                } else {
                    Point3::new(x, bottom, z)
                };
                tower.leaders.push(Leader {
                    element: e.name.text.clone(),
                    from: Point3::new(px, h, z),
                    to: top,
                });
            }
            let text = match (managed, count) {
                (false, _) if d.autoscale => format!("×{count} autoscale"),
                (false, _) => format!("×{count}"),
                (true, _) if d.primary_replica => "primary + replica".to_owned(),
                (true, 1) => "managed".to_owned(),
                (true, _) => format!("{count} instances"),
            };
            tower.tags.push(Tag {
                element: e.name.text.clone(),
                text,
                position: Point3::new(x + shift, 0.05, z + 0.85),
            });
        }
    }

    // Pipes: one per relation, routed in grid space.
    let index: Vec<usize> = (0..n).filter(|&i| cell[i].is_some()).collect();
    let cells: Vec<Cell> = index
        .iter()
        .filter_map(|&i| {
            cell[i].map(|(c, r, t)| Cell {
                c,
                r,
                t,
                shape: shape(model.elements[i].kind),
            })
        })
        .collect();
    let slot = |i: usize| index.iter().position(|&k| k == i);
    let mut edges: Vec<pipes::Edge> = Vec::new();
    for (k, rel) in model.relations.iter().enumerate() {
        let Some(Some((a, b))) = resolved.relation_ends.get(k) else {
            continue;
        };
        let (Some(from), Some(to)) = (slot(a.index()), slot(b.index())) else {
            continue;
        };
        let base = format!("{}>{}", rel.from.text, rel.to.text);
        let mut id = base.clone();
        let mut dup = 2;
        while edges.iter().any(|e| e.id == id) {
            id = format!("{base}#{dup}");
            dup += 1;
        }
        edges.push(pipes::Edge {
            id,
            from,
            to,
            is_async: rel.style == saxl_core::Style::Async,
        });
    }
    let plan = pipes::plan(&cells, &edges);
    let name = |slot: usize| model.elements[index[slot]].name.text.clone();
    let to_world = |points: &[P]| points.iter().map(|&p| world(p)).collect::<Vec<_>>();
    for p in &plan.pipes {
        tower.pipes.push(Pipe {
            id: p.id.clone(),
            from: name(p.from),
            to: name(p.to),
            is_async: p.is_async,
            points: to_world(&p.points),
            route: to_world(&p.route),
            trunk: p.trunk.map(name),
            radius: PIPE_R,
        });
    }
    for t in &plan.trunks {
        tower.trunks.push(Trunk {
            hub: name(t.hub),
            stub: to_world(&t.stub),
            run: to_world(&t.run),
            pipes: t.pipes.clone(),
            radius: TRUNK_R,
        });
    }
    tower.clashes = plan.clashes;
    tower
}
