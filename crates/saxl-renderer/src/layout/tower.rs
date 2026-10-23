//! Tower layout (3D, world units, y up). The deployment view's 2D layout is
//! reused as the floor plan: pixels divided by 60, 2D (x, y) mapped to 3D
//! (x, z), centered on the origin. Containers float above their workloads on
//! pillars, the context shell surrounds them, and actors and externals sit
//! outside it.

use indexmap::IndexMap;
use saxl_core::{ElementKind, Placement};
use saxl_validator::ValidatedModel;

use crate::layout::{deployment, node_size};
use crate::project::{self, PNode};
use crate::scene::{
    Box3, Fill, FillRole, GroupKind, Label3, NodeKind, Point3, Polyline3, Rect, Segment3, Shell,
    Tower,
};

/// Pixels per world unit.
const SCALE: f64 = 60.0;
const REGION_H: f64 = 0.06;
const GROUP_H: f64 = 0.03;
const WORKLOAD_H: f64 = 0.18;
const POD: f64 = 0.72;
const POD_H: f64 = 0.18;
const POD_GAP: f64 = 0.2;
/// Height of the container level.
const LEVEL: f64 = 3.4;
const SERVICE_H: f64 = 0.8;
const STORE_H: f64 = 1.1;
const CONTAINER_D: f64 = 1.2;
const SHELL_MARGIN: f64 = 1.5;
const OUTSIDE_MARGIN: f64 = 2.0;
const SHELL_BOTTOM: f64 = -0.3;
const SHELL_TOP: f64 = 5.6;
const INFRA_Y: f64 = 0.1;

fn container_height(kind: ElementKind) -> f64 {
    if kind == ElementKind::Store {
        STORE_H
    } else {
        SERVICE_H
    }
}

/// A box for an element-like node, sized by the 2D label metric.
fn labeled_box(n: &PNode, id: &str, center: Point3, h: f64, d: f64) -> Box3 {
    let (w, _) = node_size(n);
    Box3 {
        id: id.to_owned(),
        kind: n.kind,
        center,
        size: Point3::new(w / SCALE, h, d),
        label: Some(n.label.clone()),
        sublabel: n.sublabel.clone(),
    }
}

pub(crate) fn layout(vm: &ValidatedModel) -> Tower {
    let model = vm.model();
    let resolved = vm.resolved();
    let floor = deployment::layout(project::deployment(vm));
    let (cx, cy) = (floor.size.w / 2.0, floor.size.h / 2.0);
    let to3 = |x: f64, y: f64, height: f64| Point3::new((x - cx) / SCALE, height, (y - cy) / SCALE);
    let slab =
        |id: String, kind: NodeKind, r: &Rect, bottom: f64, h: f64, label: Option<String>| Box3 {
            id,
            kind,
            center: to3(r.x + r.w / 2.0, r.y + r.h / 2.0, bottom + h / 2.0),
            size: Point3::new(r.w / SCALE, h, r.h / SCALE),
            label,
            sublabel: None,
        };

    let mut tower = Tower {
        slabs: Vec::new(),
        pods: Vec::new(),
        containers: Vec::new(),
        pillars: Vec::new(),
        shell: Shell {
            radius: 0.0,
            y_min: SHELL_BOTTOM,
            y_max: SHELL_TOP,
        },
        outside: Vec::new(),
        infra: Vec::new(),
        context_links: Vec::new(),
        fills: Vec::new(),
        flows: Vec::new(),
        labels: Vec::new(),
    };

    // Floor: regions, groups, and their captions.
    let mut region_rects: IndexMap<String, Rect> = IndexMap::new();
    for g in &floor.groups {
        let (bottom, h, kind_label) = match g.kind {
            GroupKind::Region => (0.0, REGION_H, "region"),
            _ => (REGION_H, GROUP_H, "group"),
        };
        if g.kind == GroupKind::Region {
            region_rects.insert(g.id.clone(), g.rect);
        }
        tower.slabs.push(slab(
            format!("{kind_label}:{}", g.id),
            NodeKind::Floor,
            &g.rect,
            bottom,
            h,
            Some(g.label.clone()),
        ));
        tower.labels.push(Label3 {
            text: g.label.clone(),
            position: to3(g.rect.x + 12.0, g.rect.y + 12.0, bottom + h + 0.05),
        });
    }

    // Workloads and pods. `pod_tops[element]` lists where each pod's pillar starts.
    let workload_bottom = REGION_H + GROUP_H;
    let pod_y = workload_bottom + WORKLOAD_H + POD_H / 2.0;
    let mut pod_tops: Vec<Vec<Point3>> = vec![Vec::new(); model.elements.len()];
    let mut footprints: IndexMap<usize, Rect> = IndexMap::new();
    let mut infra_by_region: IndexMap<String, Vec<(Rect, PNode)>> = IndexMap::new();
    let deploy_nodes = project::deployment(vm);
    let mut infra_nodes = deploy_nodes
        .iter()
        .flat_map(|r| r.infra.iter().map(move |n| (r.id.clone(), n)));
    for node in &floor.nodes {
        if node.kind == NodeKind::Infra {
            if let Some((region, p)) = infra_nodes.next() {
                infra_by_region
                    .entry(region)
                    .or_default()
                    .push((node.rect, p.clone()));
            }
            continue;
        }
        let (region_name, name) = node.id.rsplit_once('/').unwrap_or(("", &node.id));
        let Some(id) = resolved.element(name) else {
            continue;
        };
        let Some(region) = model
            .regions
            .iter()
            .position(|r| r.name.text == region_name)
        else {
            continue;
        };
        let replicas = resolved.deployments[id.index()]
            .iter()
            .find(|d| d.region == region)
            .map_or(1, |d| d.replicas);
        tower.slabs.push(slab(
            format!("workload:{}", node.id),
            node.kind,
            &node.rect,
            workload_bottom,
            WORKLOAD_H,
            None,
        ));
        footprints.entry(id.index()).or_insert(node.rect);
        let w = node.rect.w / SCALE;
        let n = replicas.max(1) as f64;
        let spacing = ((w - POD_GAP) / n).min(POD + POD_GAP);
        let size = (spacing - 0.1).clamp(0.1, POD);
        let center = to3(
            node.rect.x + node.rect.w / 2.0,
            node.rect.y + node.rect.h / 2.0,
            pod_y,
        );
        for i in 0..replicas.max(1) {
            let x = center.x + (i as f64 - (n - 1.0) / 2.0) * spacing;
            tower.pods.push(Box3 {
                id: format!("pod:{}#{i}", node.id),
                kind: node.kind,
                center: Point3::new(x, pod_y, center.z),
                size: Point3::new(size, POD_H, size),
                label: None,
                sublabel: None,
            });
            pod_tops[id.index()].push(Point3::new(x, pod_y + POD_H / 2.0, center.z));
        }
    }

    // Containers: above the first workload, or in a row behind the floor if
    // the element is not deployed anywhere.
    let floor_back = floor.size.h / 2.0 / SCALE;
    let mut undeployed_x = -floor.size.w / 2.0 / SCALE;
    for (i, e) in model.elements.iter().enumerate() {
        if !e.kind.is_internal() {
            continue;
        }
        let pnode = project::element_node(model, i);
        let h = container_height(e.kind);
        let center = match footprints.get(&i) {
            Some(r) => to3(r.x + r.w / 2.0, r.y + r.h / 2.0, LEVEL),
            None => {
                let w = node_size(&pnode).0 / SCALE;
                let c = Point3::new(undeployed_x + w / 2.0, LEVEL, floor_back + 1.6);
                undeployed_x += w + 0.4;
                c
            }
        };
        let b = labeled_box(&pnode, &e.name.text, center, h, CONTAINER_D);
        for top in &pod_tops[i] {
            tower.pillars.push(Segment3 {
                from: *top,
                to: Point3::new(top.x, LEVEL - h / 2.0, top.z),
            });
        }
        // Pods in other regions reach up to the container's center instead.
        for p in tower.pillars.iter_mut().rev().take(pod_tops[i].len()) {
            let inside = (p.from.x - center.x).abs() <= b.size.x / 2.0
                && (p.from.z - center.z).abs() <= b.size.z / 2.0;
            if !inside {
                p.to = Point3::new(center.x, LEVEL - h / 2.0, center.z);
            }
        }
        tower.containers.push(b);
    }

    // Shell: around the floor and every container.
    let mut reach = (floor.size.w / 2.0 / SCALE).hypot(floor.size.h / 2.0 / SCALE);
    for c in &tower.containers {
        let corner = (c.center.x.abs() + c.size.x / 2.0).hypot(c.center.z.abs() + c.size.z / 2.0);
        reach = reach.max(corner);
    }
    tower.shell.radius = reach + SHELL_MARGIN;

    // Actors on the left half of a circle, externals on the right.
    let ring = tower.shell.radius + OUTSIDE_MARGIN;
    for (kind, start) in [
        (ElementKind::Actor, 90.0_f64),
        (ElementKind::External, -90.0),
    ] {
        let members: Vec<usize> = (0..model.elements.len())
            .filter(|&i| model.elements[i].kind == kind)
            .collect();
        let n = members.len() as f64;
        for (k, &i) in members.iter().enumerate() {
            let theta = (start + 180.0 * (k as f64 + 1.0) / (n + 1.0)).to_radians();
            let center = Point3::new(ring * theta.cos(), LEVEL, -ring * theta.sin());
            let pnode = project::element_node(model, i);
            tower.outside.push(labeled_box(
                &pnode,
                &model.elements[i].name.text,
                center,
                SERVICE_H,
                CONTAINER_D,
            ));
        }
    }

    // Infrastructure nodes: on the floor, left of their region (the actors' side).
    for (region, nodes) in infra_by_region {
        let Some(r) = region_rects.get(&region) else {
            continue;
        };
        let left = (r.x - cx) / SCALE;
        for (k, (rect, pnode)) in nodes.iter().enumerate() {
            let w = rect.w / SCALE;
            let z = (r.y - cy) / SCALE + 0.8 + 1.4 * k as f64;
            tower.infra.push(labeled_box(
                pnode,
                &pnode.label,
                Point3::new(left - 1.2 - w / 2.0, INFRA_Y, z),
                0.2,
                1.0,
            ));
        }
    }

    let center_of = |list: &[Box3], id: &str| list.iter().find(|b| b.id == id).map(|b| b.center);

    // Context links: outside elements to the containers they relate to.
    let mut linked: Vec<(usize, usize)> = Vec::new();
    for ends in resolved.relation_ends.iter().flatten() {
        let (f, t) = (ends.0.index(), ends.1.index());
        let (outside, inside) = match (
            model.elements[f].kind.is_internal(),
            model.elements[t].kind.is_internal(),
        ) {
            (false, true) => (f, t),
            (true, false) => (t, f),
            _ => continue,
        };
        if linked.contains(&(outside, inside)) {
            continue;
        }
        linked.push((outside, inside));
        let from = center_of(&tower.outside, &model.elements[outside].name.text);
        let to = center_of(&tower.containers, &model.elements[inside].name.text);
        if let (Some(from), Some(to)) = (from, to) {
            tower.context_links.push(Segment3 { from, to });
        }
    }

    // Data at rest: one fill per (store, role).
    let mut fills: IndexMap<(String, bool), Vec<String>> = IndexMap::new();
    for d in &model.data {
        let entity = d.name.text.clone();
        match &d.placement {
            Placement::In { store, cached } => {
                fills
                    .entry((store.text.clone(), true))
                    .or_default()
                    .push(entity.clone());
                for c in cached {
                    fills
                        .entry((c.text.clone(), false))
                        .or_default()
                        .push(entity.clone());
                }
            }
            Placement::Via { store, .. } => fills
                .entry((store.text.clone(), true))
                .or_default()
                .push(entity),
        }
    }
    for ((store, truth), entities) in fills {
        let Some(b) = tower.containers.iter().find(|b| b.id == store) else {
            continue;
        };
        let scale = if truth { 0.78 } else { 0.62 };
        tower.fills.push(Fill {
            store: store.clone(),
            role: if truth {
                FillRole::Truth
            } else {
                FillRole::Cache
            },
            entities,
            geometry: Box3 {
                id: format!("fill:{store}:{}", if truth { "truth" } else { "cache" }),
                kind: NodeKind::Store,
                center: b.center,
                size: Point3::new(b.size.x * scale, b.size.y * scale, b.size.z * scale),
                label: None,
                sublabel: None,
            },
        });
    }

    // Data in flight: queue → consumer → store for each `via` entity.
    for d in &model.data {
        let Placement::Via { queue, store } = &d.placement else {
            continue;
        };
        let relates = |from: &str, to: &str| {
            model
                .relations
                .iter()
                .any(|r| r.from.text == from && r.to.text == to)
        };
        let consumers: Vec<&str> = model
            .relations
            .iter()
            .filter(|r| r.from.text == queue.text)
            .map(|r| r.to.text.as_str())
            .collect();
        let consumer = consumers
            .iter()
            .find(|c| relates(c, &store.text))
            .or(consumers.first())
            .copied();
        let mut points = Vec::new();
        for id in std::iter::once(queue.text.as_str())
            .chain(consumer)
            .chain([store.text.as_str()])
        {
            if let Some(c) = center_of(&tower.containers, id) {
                points.push(c);
            }
        }
        if points.len() >= 2 {
            tower.flows.push(Polyline3 {
                id: d.name.text.clone(),
                points,
            });
        }
    }

    tower
}
