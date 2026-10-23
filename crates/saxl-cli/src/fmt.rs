//! `saxl fmt`: rewrite files in canonical form.

use std::path::PathBuf;
use std::process::ExitCode;

use saxl_core::FileId;

use crate::{CliError, load, report};

#[derive(clap::Args)]
pub struct Args {
    /// Files to format.
    #[arg(required = true)]
    files: Vec<PathBuf>,
    /// Don't write; list files that need formatting and fail if there are any.
    #[arg(long)]
    check: bool,
}

pub fn run(args: &Args) -> Result<ExitCode, CliError> {
    let mut failed = false;
    for path in &args.files {
        let source = load::read(path, FileId(0))?;
        let formatted = match saxl_parser::format(&source.text) {
            Ok(text) => text,
            Err(diagnostics) => {
                eprint!(
                    "{}",
                    report::human(&source, &diagnostics, crate::color_stderr())
                );
                eprintln!(
                    "error: not formatting {} because it doesn't parse",
                    source.name
                );
                failed = true;
                continue;
            }
        };
        if formatted == source.text {
            continue;
        }
        if args.check {
            println!("{} needs formatting", source.name);
            failed = true;
        } else {
            std::fs::write(path, formatted)
                .map_err(|e| CliError(format!("can't write {}: {e}", source.name)))?;
            println!("formatted {}", source.name);
        }
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
