//! `saxl serve`: a local page that updates whenever the file is saved.
//!
//! The page loads `/data` (scenes, or a diagnostics report when the file has
//! errors) and listens on `/events`, a server-sent-events stream that says
//! "change" when the watcher sees the file change. It then fetches `/data`
//! again and swaps the scenes in place, so the view and camera are kept.

use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, Json};
use axum::routing::get;
use futures_util::stream::{self, Stream};
use notify::{EventKind, RecursiveMode, Watcher};
use saxl_core::{Diagnostic, FileId};
use saxl_renderer::Scene;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::{CliError, load, report};

#[derive(clap::Args)]
pub struct Args {
    /// The file to serve.
    file: PathBuf,
    /// Port on 127.0.0.1.
    #[arg(long, default_value_t = 4848)]
    port: u16,
}

struct App {
    path: PathBuf,
    page: String,
    changes: broadcast::Sender<()>,
}

pub fn run(args: &Args) -> Result<ExitCode, CliError> {
    // Fail early, with exit code 2, if the file can't be read.
    let source = load::read(&args.file, FileId(0))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError(format!("can't start the server runtime: {e}")))?;
    runtime.block_on(serve(args, &source.name))
}

async fn serve(args: &Args, name: &str) -> Result<ExitCode, CliError> {
    let (changes, _) = broadcast::channel(16);
    // Dropping the watcher stops it, so keep it alive for the server's lifetime.
    let _watcher = watch(&args.file, changes.clone())?;
    let app = Arc::new(App {
        path: args.file.clone(),
        page: page(name),
        changes,
    });
    let router = Router::new()
        .route("/", get(index))
        .route("/data", get(data))
        .route("/events", get(events))
        .with_state(app);
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, args.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| CliError(format!("can't listen on {addr}: {e}")))?;
    println!("serving {name} at http://{addr}/ (press Ctrl+C to stop)");
    axum::serve(listener, router)
        .await
        .map_err(|e| CliError(format!("server error: {e}")))?;
    Ok(ExitCode::SUCCESS)
}

/// Watches the file's directory, since editors often save by replacing the
/// file, and signals `changes` for events on the file's name.
fn watch(
    path: &Path,
    changes: broadcast::Sender<()>,
) -> Result<notify::RecommendedWatcher, CliError> {
    let err = |e: &dyn std::fmt::Display| CliError(format!("can't watch {}: {e}", path.display()));
    let target = std::fs::canonicalize(path).map_err(|e| err(&e))?;
    let name = target.file_name().map(ToOwned::to_owned);
    let dir = target.parent().unwrap_or(Path::new("/")).to_owned();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        let ours = event.paths.iter().any(|p| p.file_name() == name.as_deref());
        if ours && !matches!(event.kind, EventKind::Access(_)) {
            // No receivers just means no page is open.
            let _ = changes.send(());
        }
    })
    .map_err(|e| err(&e))?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| err(&e))?;
    Ok(watcher)
}

async fn index(State(app): State<Arc<App>>) -> Html<String> {
    Html(app.page.clone())
}

/// The current state of the file.
#[derive(Serialize)]
struct Data {
    /// Every view, or null when the file has errors.
    scenes: Option<Vec<Scene>>,
    diagnostics: Vec<Diagnostic>,
    /// Human-readable diagnostics, shown when there are errors.
    report: String,
}

async fn data(State(app): State<Arc<App>>) -> Json<Data> {
    Json(match load::load(&app.path, FileId(0)) {
        Ok(loaded) => {
            let diagnostics = loaded.diagnostics();
            let report = report::human(&loaded.source, &diagnostics, false);
            let scenes = loaded
                .validation
                .and_then(|v| v.model)
                .map(|model| saxl_renderer::scenes(&model));
            Data {
                scenes,
                diagnostics,
                report,
            }
        }
        Err(CliError(message)) => Data {
            scenes: None,
            diagnostics: Vec::new(),
            report: message,
        },
    })
}

async fn events(State(app): State<Arc<App>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let changes = app.changes.subscribe();
    let stream = stream::unfold(changes, |mut changes| async move {
        match changes.recv().await {
            // Missed some changes: one refresh covers them all.
            Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                Some((Ok(Event::default().data("change")), changes))
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn page(name: &str) -> String {
    let title = name.replace('&', "&amp;").replace('<', "&lt;");
    let viewer = saxl_renderer::VIEWER_JS.replace("</script", "<\\/script");
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · saxl serve</title>
<style>
html,body{{margin:0;height:100%;background:#f8f9fb}}
#saxl{{height:100%;transition:opacity .2s}}
body.stale #saxl{{opacity:.45}}
#errors{{position:fixed;left:16px;right:16px;bottom:16px;max-height:45%;overflow:auto;margin:0;padding:12px 16px;border:1px solid #d4552b;border-radius:8px;background:#fff;color:#151a22;font:12px/1.45 ui-monospace,Menlo,Consolas,monospace;white-space:pre-wrap}}
@media (prefers-color-scheme:dark){{html,body{{background:#151a21}}#errors{{background:#1c222b;color:#e6e9ef;border-color:#f07a52}}}}
</style></head>
<body>
<div id="saxl"></div>
<pre id="errors" hidden></pre>
<script>{viewer}</script>
<script>
(function () {{
  var el = document.getElementById("saxl"), errors = document.getElementById("errors");
  var viewer = null, latest = 0;
  function refresh() {{
    var seq = ++latest;
    fetch("data").then(function (r) {{ return r.json(); }}).then(function (d) {{
      if (seq !== latest) return;
      if (d.scenes) {{
        if (viewer) {{ viewer.setScenes(d.scenes); viewer.setDiagnostics(d.diagnostics); }}
        else viewer = saxlViewer.mount(el, d.scenes, {{ initialView: {{ kind: "tower" }}, diagnostics: d.diagnostics }});
        document.body.classList.remove("stale");
        errors.hidden = true;
      }} else {{
        errors.textContent = d.report;
        errors.hidden = false;
        document.body.classList.add("stale");
      }}
    }});
  }}
  refresh();
  new EventSource("events").onmessage = refresh;
}})();
</script>
</body></html>
"#
    )
}
