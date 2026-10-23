//! Scene JSON and SVG snapshots for every view of Snip, plus determinism and
//! HTML embedding checks.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use saxl_core::FileId;
use saxl_renderer::{HtmlOptions, ViewKind};
use saxl_validator::ValidatedModel;

fn model(src: &str) -> ValidatedModel {
    let parsed = saxl_parser::parse(src, FileId(0));
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let validation = saxl_validator::validate(parsed.model);
    validation.model.expect("model validates")
}

fn snip() -> ValidatedModel {
    model(include_str!("../../../examples/snip.saxl"))
}

#[test]
fn snip_scene_snapshots() {
    let m = snip();
    for view in saxl_renderer::views(&m) {
        let scene = saxl_renderer::scene(&m, &view).unwrap();
        insta::assert_snapshot!(
            format!("scene-{}", view.slug()),
            saxl_renderer::to_json(&scene)
        );
        if view != ViewKind::Tower {
            let svg = saxl_renderer::to_svg(&scene).unwrap();
            insta::assert_snapshot!(format!("svg-{}", view.slug()), svg);
        }
    }
}

#[test]
fn every_view_is_rendered() {
    let scenes = saxl_renderer::scenes(&snip());
    let slugs: Vec<_> = scenes.iter().map(|s| s.view.slug()).collect();
    assert_eq!(
        slugs,
        [
            "context",
            "containers",
            "runtime-redirect_miss",
            "runtime-redirect_hit",
            "data",
            "deployment",
            "tower"
        ]
    );
}

#[test]
fn output_is_deterministic() {
    let opts = HtmlOptions {
        title: "Snip".to_owned(),
        initial_view: ViewKind::Context,
        diagnostics: Vec::new(),
    };
    let first = saxl_renderer::to_html(&snip(), &opts);
    let second = saxl_renderer::to_html(&snip(), &opts);
    assert_eq!(first, second);
}

#[test]
fn hit_path_skips_postgres() {
    let m = snip();
    let view = ViewKind::Runtime {
        scenario: "redirect_hit".to_owned(),
    };
    let scene = saxl_renderer::scene(&m, &view).unwrap();
    let steps = &scene.timelines[0].steps;
    assert!(
        steps
            .iter()
            .all(|s| s.from != "Postgres" && s.to != "Postgres")
    );
    // Visitor -> Edge -> Redirect is one step through the CDN.
    assert_eq!(steps[0].via, ["Edge"]);
    // The three parallel branches share a playback slot.
    let last = &steps[steps.len() - 2..];
    assert_eq!(last[0].index, last[1].index);
}

#[test]
fn unknown_scenario_and_tower_svg() {
    let m = snip();
    let missing = ViewKind::Runtime {
        scenario: "nope".to_owned(),
    };
    assert_eq!(
        saxl_renderer::scene(&m, &missing),
        Err(saxl_renderer::RenderError::UnknownScenario(
            "nope".to_owned()
        ))
    );
    let scene = saxl_renderer::scene(&m, &ViewKind::Tower).unwrap();
    assert_eq!(
        saxl_renderer::to_svg(&scene),
        Err(saxl_renderer::RenderError::NotTwoDimensional)
    );
}

#[test]
fn html_escapes_script_end_tags() {
    let src = "system S \"</script><b>x\"\nactor U\nservice A \"</script>\"\nU -> A\n\
               deploy region r {\n  cluster c {\n    A x2\n  }\n}\nallow W102 \"test\"\n";
    let opts = HtmlOptions {
        title: "</title>".to_owned(),
        initial_view: ViewKind::Containers,
        diagnostics: Vec::new(),
    };
    let html = saxl_renderer::to_html(&model(src), &opts);
    let data_start = html.find("id=\"saxl-data\">").unwrap();
    let data_end = data_start + html[data_start..].find("</script>").unwrap();
    let data = &html[data_start..data_end];
    assert!(data.contains("<\\/script>"));
    assert!(html.contains("<title>&lt;/title&gt;</title>"));
    let json = &data["id=\"saxl-data\">".len()..];
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(parsed["initialView"]["kind"], "containers");
    assert_eq!(parsed["scenes"][0]["view"]["kind"], "context");
}

/// End to end: the default HTML opens on a tower with 7 containers,
/// 4 outside elements, 1 infrastructure node, and 2 scenarios.
#[test]
fn snip_html_opens_on_the_tower() {
    let opts = HtmlOptions {
        title: "Snip".to_owned(),
        initial_view: ViewKind::Tower,
        diagnostics: Vec::new(),
    };
    let html = saxl_renderer::to_html(&snip(), &opts);
    let marker = "id=\"saxl-data\">";
    let start = html.find(marker).unwrap() + marker.len();
    let end = start + html[start..].find("</script>").unwrap();
    let data: serde_json::Value = serde_json::from_str(&html[start..end]).unwrap();
    assert_eq!(data["initialView"]["kind"], "tower");
    let tower_scene = data["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["view"]["kind"] == "tower")
        .unwrap();
    let tower = &tower_scene["tower"];
    let count = |key: &str| tower[key].as_array().unwrap().len();
    assert_eq!(count("containers"), 7);
    assert_eq!(count("outside"), 4);
    assert_eq!(count("infra"), 1);
    assert_eq!(tower_scene["timelines"].as_array().unwrap().len(), 2);
}

fn shopfront() -> ValidatedModel {
    model(include_str!("../../../examples/shopfront.saxl"))
}

/// Checks what every tower layout guarantees: tiers stack in declaration
/// order, no two containers share a cell, actors stand left of the grid and
/// externals right of it, every replica has a leader up to its container,
/// every relation is a pipe, and no two unrelated pipes touch.
fn assert_tower_invariants(m: &ValidatedModel, tiers: &[&str]) -> saxl_renderer::Tower {
    let scene = saxl_renderer::scene(m, &ViewKind::Tower).unwrap();
    let t = scene.tower.unwrap();
    let labels: Vec<_> = t.tiers.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(labels, tiers);
    // Declared first, stacked highest.
    for pair in t.tiers.windows(2) {
        assert!(
            pair[0].min.y > pair[1].min.y,
            "{} above {}",
            pair[0].label,
            pair[1].label
        );
    }
    for (i, a) in t.containers.iter().enumerate() {
        for b in &t.containers[i + 1..] {
            let same =
                (a.center.x - b.center.x).abs() < 1e-6 && (a.center.z - b.center.z).abs() < 1e-6;
            assert!(!same, "{} and {} share a cell", a.id, b.id);
        }
    }
    let min_x = t
        .containers
        .iter()
        .map(|c| c.center.x)
        .fold(f64::MAX, f64::min);
    let max_x = t
        .containers
        .iter()
        .map(|c| c.center.x)
        .fold(f64::MIN, f64::max);
    for b in &t.outside {
        if b.kind == saxl_renderer::NodeKind::Actor {
            assert!(b.center.x < min_x, "{}", b.id);
        } else {
            assert!(b.center.x > max_x, "{}", b.id);
        }
    }
    assert_eq!(t.leaders.len(), t.pods.len());
    for l in &t.leaders {
        let c = t.containers.iter().find(|c| c.id == l.element).unwrap();
        assert!(
            (l.to.x - c.center.x).abs() <= c.size.x / 2.0,
            "{}",
            l.element
        );
        assert!(
            (l.to.z - c.center.z).abs() <= c.size.z / 2.0,
            "{}",
            l.element
        );
    }
    assert_eq!(t.pipes.len(), m.model().relations.len());
    assert_eq!(t.clashes, 0);
    t
}

#[test]
fn tower_structure() {
    let t = assert_tower_invariants(&snip(), &["k8s cluster", "managed"]);
    // One pod per replica: Redirect ×3, Dashboard ×2, LinksAPI ×2, Worker ×1;
    // one pad per managed instance: Postgres ×2 (primary+replica), Redis, Clicks.
    assert_eq!(t.pods.len(), 12);
    assert_eq!(t.pods.iter().filter(|p| p.managed).count(), 4);
    // Snip has no hub with four outgoing relations.
    assert!(t.trunks.is_empty());
}

#[test]
fn shopfront_tower_structure() {
    let m = shopfront();
    let t = assert_tower_invariants(&m, &["edge cluster", "domain cluster", "managed"]);
    assert_eq!(t.containers.len(), 21);
    assert_eq!(t.outside.len(), 5);
    let mut hubs: Vec<_> = t.trunks.iter().map(|x| x.hub.as_str()).collect();
    hubs.sort_unstable();
    assert_eq!(hubs, ["Checkout", "Gateway"]);
    // Every branch names its trunk, and every trunk lists its branches.
    for trunk in &t.trunks {
        for id in &trunk.pipes {
            let pipe = t.pipes.iter().find(|p| &p.id == id).unwrap();
            assert_eq!(pipe.trunk.as_deref(), Some(trunk.hub.as_str()));
        }
    }
    let scene = saxl_renderer::scene(&m, &ViewKind::Tower).unwrap();
    insta::assert_snapshot!("shopfront-scene-tower", saxl_renderer::to_json(&scene));
}

#[test]
fn anchors_cover_declarations_and_references() {
    let m = snip();
    let scene = saxl_renderer::scene(&m, &ViewKind::Containers).unwrap();
    let postgres: Vec<_> = scene
        .anchors
        .iter()
        .filter(|a| a.node == "Postgres")
        .collect();
    // Declaration, 3 relations, 3 data placements, 1 workload, 2 scenario hops.
    assert_eq!(postgres.len(), 10);
    let decl = m
        .model()
        .elements
        .iter()
        .find(|e| e.name.text == "Postgres")
        .unwrap()
        .span;
    assert_eq!(postgres[0].span, decl);
}
