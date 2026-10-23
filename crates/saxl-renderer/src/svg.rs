//! Plain SVG for the 2D views, using the scene's geometry.

use std::fmt::Write;

use crate::RenderError;
use crate::layout::{BADGE_ROW, FIELD_LINE, PAD, SMALL_PX, badge_text};
use crate::scene::{
    EdgeStyle, GroupKind, NodeKind, Point2, Scene, SceneEdge, SceneGroup, SceneNode, ViewKind,
};

/// Color tokens: name, light value, dark value.
pub(crate) const TOKENS: &[(&str, &str, &str)] = &[
    ("service", "#5b57d1", "#8f8cf0"),
    ("store", "#0f8a6c", "#3fc79f"),
    ("event", "#d4552b", "#f07a52"),
    ("request", "#c98407", "#f5b742"),
    ("structure", "#7d889a", "#6d7789"),
    ("fg", "#151a22", "#e6e9ef"),
    ("muted", "#586274", "#97a1b2"),
    ("bg", "#f8f9fb", "#151a21"),
];

/// Rules that use colors. `$name` is replaced by a token's value. Values are
/// written out instead of using CSS variables, which many SVG tools ignore.
const COLOR_RULES: &str = ".bg{fill:$bg}text{fill:$fg}.sub,.group-label,.badge,.edge-label{fill:$muted}\
.node{color:$structure}.node-service{color:$service}.node-store{color:$store}.node-queue{color:$event}\
.node .base{fill:$bg}.edge{stroke:$muted}.edge-async{stroke:$event}.edge-label{stroke:$bg}\
.lifeline,.group rect{stroke:$structure}.group-parallel rect{stroke:$request;fill:$request}\
.arrow-sync,.arrow-reply{fill:$muted}.arrow-async{fill:$event}";

/// Rules that don't depend on the color scheme.
const SHAPE_RULES: &str = "text{font-size:14px}.sub,.badge,.field,.edge-label,.group-label{font-size:12px}\
.badge{font-weight:600}.node .tint{fill:currentColor;fill-opacity:.1;stroke:currentColor;stroke-width:1.5}\
.node-external .tint,.node-infra .tint{stroke-dasharray:5 3}.node-system .tint{fill-opacity:.05;stroke-width:2}\
.edge{fill:none;stroke-width:1.5}.edge-async{stroke-dasharray:6 4}.edge-reply{stroke-dasharray:2 4;opacity:.75}\
.edge-label{paint-order:stroke;stroke-width:4px}.lifeline{stroke-dasharray:4 4;opacity:.6}\
.group rect{fill:none;stroke-width:1.2}.group-cluster rect,.group-managed rect{stroke-dasharray:6 3}\
.group-parallel rect{fill-opacity:.06}";

fn colors(dark: bool) -> String {
    let mut rules = COLOR_RULES.to_owned();
    for (name, light, dark_value) in TOKENS {
        rules = rules.replace(&format!("${name}"), if dark { dark_value } else { light });
    }
    rules
}

/// The SVG stylesheet: light colors, dark overrides, and shape rules.
pub(crate) fn style() -> String {
    format!(
        "{}@media (prefers-color-scheme:dark){{{}}}{SHAPE_RULES}",
        colors(false),
        colors(true)
    )
}

fn num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 {
        "0".to_owned()
    } else {
        r.to_string()
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn style_name(style: EdgeStyle) -> &'static str {
    match style {
        EdgeStyle::Sync => "sync",
        EdgeStyle::Async => "async",
        EdgeStyle::Reply => "reply",
    }
}

fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Actor => "actor",
        NodeKind::External => "external",
        NodeKind::Service => "service",
        NodeKind::Store => "store",
        NodeKind::Queue => "queue",
        NodeKind::System => "system",
        NodeKind::Infra => "infra",
        NodeKind::DataCard => "data_card",
        NodeKind::Floor => "floor",
    }
}

fn group_name(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Region => "region",
        GroupKind::Cluster => "cluster",
        GroupKind::Managed => "managed",
        GroupKind::Parallel => "parallel",
    }
}

/// Renders a 2D scene as a standalone SVG document.
pub fn to_svg(scene: &Scene) -> Result<String, RenderError> {
    if scene.view == ViewKind::Tower {
        return Err(RenderError::NotTwoDimensional);
    }
    let (w, h) = (num(scene.size.w), num(scene.size.h));
    let mut s = String::new();
    // Writing to a String never fails.
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="ui-monospace, Menlo, Consolas, monospace">"#
    );
    let _ = writeln!(s, "<title>{}</title>", escape(&scene.title));
    let _ = writeln!(s, "<style>{}</style>", style());
    s.push_str("<defs>");
    for style in ["sync", "async", "reply"] {
        let _ = write!(
            s,
            r#"<marker id="arrow-{style}" class="arrow-{style}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z"/></marker>"#
        );
    }
    s.push_str("</defs>\n");
    let _ = writeln!(s, r#"<rect class="bg" width="{w}" height="{h}"/>"#);

    let mut groups: Vec<&SceneGroup> = scene.groups.iter().collect();
    groups.sort_by_key(|g| g.depth);
    for g in groups {
        group(&mut s, g);
    }
    if matches!(scene.view, ViewKind::Runtime { .. }) {
        for n in &scene.nodes {
            let x = num(n.rect.center().x);
            let _ = writeln!(
                s,
                r#"<line class="lifeline" x1="{x}" y1="{}" x2="{x}" y2="{}"/>"#,
                num(n.rect.bottom()),
                num(scene.size.h - PAD)
            );
        }
    }
    for e in &scene.edges {
        edge(&mut s, e);
    }
    for n in &scene.nodes {
        node(&mut s, n);
    }
    s.push_str("</svg>\n");
    Ok(s)
}

fn group(s: &mut String, g: &SceneGroup) {
    let r = &g.rect;
    let _ = writeln!(
        s,
        r#"<g class="group group-{}"><rect x="{}" y="{}" width="{}" height="{}" rx="8"/><text class="group-label" x="{}" y="{}">{}</text></g>"#,
        group_name(g.kind),
        num(r.x),
        num(r.y),
        num(r.w),
        num(r.h),
        num(r.x + 10.0),
        num(r.y + 18.0),
        escape(&g.label)
    );
}

fn edge(s: &mut String, e: &SceneEdge) {
    let style = style_name(e.style);
    let points: Vec<String> = e
        .points
        .iter()
        .map(|p| format!("{},{}", num(p.x), num(p.y)))
        .collect();
    let _ = writeln!(
        s,
        r#"<polyline class="edge edge-{style}" points="{}" marker-end="url(#arrow-{style})"/>"#,
        points.join(" ")
    );
    if let Some(label) = &e.label {
        let at = label_anchor(&e.points);
        let _ = writeln!(
            s,
            r#"<text class="edge-label" x="{}" y="{}" text-anchor="middle">{}</text>"#,
            num(at.x),
            num(at.y - 6.0),
            escape(label)
        );
    }
}

/// The midpoint of the polyline's middle segment.
pub(crate) fn label_anchor(points: &[Point2]) -> Point2 {
    if points.len() < 2 {
        return points.first().copied().unwrap_or_default();
    }
    let i = (points.len() - 1) / 2;
    let (a, b) = (points[i], points[i + 1]);
    Point2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

fn node(s: &mut String, n: &SceneNode) {
    let r = &n.rect;
    let radius = match n.kind {
        NodeKind::Actor => 18.0,
        NodeKind::System => 12.0,
        _ => 6.0,
    };
    let rect = format!(
        r#"x="{}" y="{}" width="{}" height="{}" rx="{radius}""#,
        num(r.x),
        num(r.y),
        num(r.w),
        num(r.h)
    );
    let _ = write!(
        s,
        r#"<g class="node node-{}" data-id="{}"><rect class="base" {rect}/><rect class="tint" {rect}/>"#,
        kind_name(n.kind),
        escape(&n.id)
    );
    let cx = num(r.center().x);
    let left = n.kind == NodeKind::DataCard;
    let (tx, anchor) = if left {
        (num(r.x + 16.0), "start")
    } else {
        (cx.clone(), "middle")
    };
    let (title_y, sub_y) = if n.sublabel.is_some() {
        (22.0, 40.0)
    } else {
        (25.0, 0.0)
    };
    let _ = write!(
        s,
        r#"<text x="{tx}" y="{}" text-anchor="{anchor}" font-weight="600">{}</text>"#,
        num(r.y + title_y),
        escape(&n.label)
    );
    if let Some(sub) = &n.sublabel {
        let _ = write!(
            s,
            r#"<text class="sub" x="{tx}" y="{}" text-anchor="{anchor}">{}</text>"#,
            num(r.y + sub_y),
            escape(sub)
        );
    }
    let mut y = r.y + if n.sublabel.is_some() { 56.0 } else { 40.0 };
    if !n.badges.is_empty() {
        let _ = write!(
            s,
            r#"<text class="badge" x="{cx}" y="{}" text-anchor="middle">{}</text>"#,
            num(y + BADGE_ROW - 8.0),
            escape(&badge_text(&n.badges))
        );
        y += BADGE_ROW;
    }
    for f in &n.fields {
        y += FIELD_LINE;
        let _ = write!(
            s,
            r#"<text class="field" x="{}" y="{}">{}</text>"#,
            num(r.x + 16.0),
            num(y - (FIELD_LINE - SMALL_PX) / 2.0),
            escape(f)
        );
    }
    s.push_str("</g>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_escaping() {
        assert_eq!(num(12.0), "12");
        assert_eq!(num(-0.001), "0");
        assert_eq!(num(1.005_1), "1.01");
        assert_eq!(escape("a<b & \"c\""), "a&lt;b &amp; &quot;c&quot;");
    }

    #[test]
    fn style_has_no_unresolved_tokens() {
        let css = style();
        assert!(!css.contains('$'), "{css}");
        assert!(css.contains("#5b57d1") && css.contains("#8f8cf0"));
    }
}
