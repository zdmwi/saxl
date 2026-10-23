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
        let svg = saxl_renderer::to_svg(&scene).unwrap();
        insta::assert_snapshot!(format!("svg-{}", view.slug()), svg);
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
            "deployment"
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
    let mut scene = saxl_renderer::scene(&m, &ViewKind::Context).unwrap();
    scene.view = ViewKind::Tower;
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
