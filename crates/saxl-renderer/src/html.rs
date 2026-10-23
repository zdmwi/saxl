//! Self-contained HTML output: every scene as JSON plus the bundled viewer.

use saxl_core::Diagnostic;
use saxl_validator::ValidatedModel;
use serde::Serialize;

use crate::{Scene, VIEWER_JS, ViewKind, scenes};

/// Options for [`to_html`].
#[derive(Debug, Clone)]
pub struct HtmlOptions {
    /// The page title.
    pub title: String,
    /// The view shown first.
    pub initial_view: ViewKind,
    /// Warnings and infos to show alongside the views.
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payload<'a> {
    scenes: Vec<Scene>,
    initial_view: &'a ViewKind,
    diagnostics: &'a [Diagnostic],
}

/// One HTML file that works offline: the scenes for every view, the
/// diagnostics, and the viewer bundle.
pub fn to_html(model: &ValidatedModel, opts: &HtmlOptions) -> String {
    let payload = Payload {
        scenes: scenes(model),
        initial_view: &opts.initial_view,
        diagnostics: &opts.diagnostics,
    };
    // `</` would let a description such as `</script>` end the block early.
    let json = serde_json::to_string(&payload)
        .unwrap_or_else(|_| "{}".to_owned())
        .replace("</", "<\\/");
    let title = escape_text(&opts.title);
    let viewer = VIEWER_JS.replace("</script", "<\\/script");
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title}</title><style>html,body{{margin:0;height:100%}}#saxl{{height:100%}}</style></head>
<body>
<div id="saxl"></div>
<script type="application/json" id="saxl-data">{json}</script>
<script>{viewer}</script>
<script>(function(){{var d=JSON.parse(document.getElementById("saxl-data").textContent);saxlViewer.mount(document.getElementById("saxl"),d.scenes,{{initialView:d.initialView,diagnostics:d.diagnostics}});}})();</script>
</body></html>
"#
    )
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
