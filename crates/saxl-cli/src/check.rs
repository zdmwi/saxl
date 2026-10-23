//! `saxl check`: parse and validate.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::ValueEnum;
use saxl_core::{FileId, Severity};

use crate::{CliError, load, report};

#[derive(clap::Args)]
pub struct Args {
    /// Files to check.
    #[arg(required = true)]
    files: Vec<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
    /// Treat a category of diagnostics as errors for the exit code.
    #[arg(long, value_enum)]
    deny: Option<Deny>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Json,
    Github,
}

#[derive(Clone, Copy, ValueEnum)]
enum Deny {
    Warnings,
}

pub fn run(args: &Args) -> Result<ExitCode, CliError> {
    let mut files = Vec::with_capacity(args.files.len());
    for (i, path) in args.files.iter().enumerate() {
        let loaded = load::load(path, FileId(u32::try_from(i).unwrap_or(u32::MAX)))?;
        let diags = loaded.diagnostics();
        files.push((loaded.source, diags));
    }

    match args.format {
        Format::Human => {
            let color = crate::color_stdout();
            let mut all = Vec::new();
            for (source, diags) in &files {
                print!("{}", report::human(source, diags, color));
                all.extend(diags.iter().cloned());
            }
            let noun = if files.len() == 1 { "file" } else { "files" };
            println!("checked {} {noun}: {}", files.len(), report::summary(&all));
        }
        Format::Json => {
            let refs: Vec<_> = files.iter().map(|(s, d)| (s, d.clone())).collect();
            println!("{}", report::json(&refs));
        }
        Format::Github => {
            for (source, diags) in &files {
                print!("{}", report::github(source, diags));
            }
        }
    }

    let failed = files.iter().flat_map(|(_, d)| d).any(|d| match d.severity {
        Severity::Error => true,
        Severity::Warning => matches!(args.deny, Some(Deny::Warnings)),
        Severity::Info => false,
    });
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
