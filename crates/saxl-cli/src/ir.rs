//! `saxl ir`: print the IR or its JSON Schema.

use std::path::PathBuf;
use std::process::ExitCode;

use saxl_core::{FileId, Model};

use crate::{CliError, load, report};

#[derive(clap::Args)]
pub struct Args {
    /// The file to parse.
    #[arg(required_unless_present = "schema")]
    file: Option<PathBuf>,
    /// Print the IR's JSON Schema instead.
    #[arg(long, conflicts_with = "file")]
    schema: bool,
}

pub fn run(args: &Args) -> Result<ExitCode, CliError> {
    let json = if args.schema {
        serde_json::to_string_pretty(&schemars::schema_for!(Model))
    } else {
        let Some(path) = &args.file else {
            return Err(CliError("a file is required".to_owned()));
        };
        let source = load::read(path, FileId(0))?;
        let parsed = saxl_parser::parse(&source.text, source.id);
        if !parsed.diagnostics.is_empty() {
            eprint!(
                "{}",
                report::human(&source, &parsed.diagnostics, crate::color_stderr())
            );
            return Ok(ExitCode::FAILURE);
        }
        serde_json::to_string_pretty(&parsed.model)
    };
    let json = json.map_err(|e| CliError(format!("can't serialize: {e}")))?;
    println!("{json}");
    Ok(ExitCode::SUCCESS)
}
