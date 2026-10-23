//! `saxl render`: write a view as HTML, SVG, or JSON.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::ValueEnum;
use saxl_core::{FileId, Severity};
use saxl_renderer::{HtmlOptions, ViewKind};

use crate::{CliError, load, report};

#[derive(clap::Args)]
pub struct Args {
    /// The file to render.
    file: PathBuf,
    /// Which view to render.
    #[arg(long, value_enum, default_value_t = View::All)]
    view: View,
    /// The scenario to show. Required for `--view runtime`.
    #[arg(long)]
    scenario: Option<String>,
    /// Output path. The extension picks the format: .html, .svg, or .json.
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum View {
    Tower,
    Context,
    Containers,
    Runtime,
    Data,
    Deployment,
    All,
}

enum Format {
    Html,
    Svg,
    Json,
}

pub fn run(args: &Args) -> Result<ExitCode, CliError> {
    let format = match args
        .output
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") => Format::Html,
        Some("svg") => Format::Svg,
        Some("json") => Format::Json,
        _ => {
            return Err(CliError(format!(
                "can't tell the format of {}; use a .html, .svg, or .json extension",
                args.output.display()
            )));
        }
    };
    let view = match (args.view, &args.scenario) {
        (View::Runtime, Some(s)) => Some(ViewKind::Runtime {
            scenario: s.clone(),
        }),
        (View::Runtime, None) => {
            return Err(CliError(
                "`--view runtime` needs `--scenario <name>`".to_owned(),
            ));
        }
        (_, Some(_)) => {
            return Err(CliError(
                "`--scenario` only applies to `--view runtime`".to_owned(),
            ));
        }
        (View::Tower, None) => Some(ViewKind::Tower),
        (View::Context, None) => Some(ViewKind::Context),
        (View::Containers, None) => Some(ViewKind::Containers),
        (View::Data, None) => Some(ViewKind::Data),
        (View::Deployment, None) => Some(ViewKind::Deployment),
        (View::All, None) => None,
    };

    let loaded = load::load(&args.file, FileId(0))?;
    let diagnostics = loaded.diagnostics();
    if !diagnostics.is_empty() {
        eprint!(
            "{}",
            report::human(&loaded.source, &diagnostics, crate::color_stderr())
        );
    }
    let model = match loaded.validation.and_then(|v| v.model) {
        Some(m) if !diagnostics.iter().any(|d| d.severity == Severity::Error) => m,
        _ => {
            eprintln!(
                "error: not rendering {} because it has errors",
                loaded.source.name
            );
            return Ok(ExitCode::FAILURE);
        }
    };

    let render_err = |e: saxl_renderer::RenderError| CliError(e.to_string());
    let output = match (format, &view) {
        (Format::Html, _) => {
            if let Some(v) = &view {
                saxl_renderer::scene(&model, v).map_err(render_err)?;
            }
            let title = model
                .model()
                .systems
                .first()
                .map_or_else(|| "saxl".to_owned(), |s| s.name.text.clone());
            saxl_renderer::to_html(
                &model,
                &HtmlOptions {
                    title,
                    initial_view: view.unwrap_or(ViewKind::Context),
                    diagnostics,
                },
            )
        }
        (Format::Svg, None) => {
            return Err(CliError(
                "SVG output shows one view; pass `--view`".to_owned(),
            ));
        }
        (Format::Svg, Some(v)) => {
            let scene = saxl_renderer::scene(&model, v).map_err(render_err)?;
            saxl_renderer::to_svg(&scene).map_err(render_err)?
        }
        (Format::Json, Some(v)) => {
            let scene = saxl_renderer::scene(&model, v).map_err(render_err)?;
            saxl_renderer::to_json(&scene)
        }
        (Format::Json, None) => serde_json::to_string_pretty(&saxl_renderer::scenes(&model))
            .map_err(|e| CliError(format!("can't serialize: {e}")))?,
    };
    std::fs::write(&args.output, output)
        .map_err(|e| CliError(format!("can't write {}: {e}", args.output.display())))?;
    println!("wrote {}", args.output.display());
    Ok(ExitCode::SUCCESS)
}
