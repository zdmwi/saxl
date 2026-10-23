//! `saxl init`: write a starter file.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::CliError;

const STARTER: &str = include_str!("../../../examples/snip.saxl");

#[derive(clap::Args)]
pub struct Args {
    /// Where to write the file.
    #[arg(default_value = "architecture.saxl")]
    path: PathBuf,
}

pub fn run(args: &Args) -> Result<ExitCode, CliError> {
    let path = &args.path;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => CliError(format!(
                "{} already exists; refusing to overwrite it",
                path.display()
            )),
            _ => CliError(format!("can't create {}: {e}", path.display())),
        })?;
    file.write_all(STARTER.as_bytes())
        .map_err(|e| CliError(format!("can't write {}: {e}", path.display())))?;
    println!("wrote {}", path.display());
    Ok(ExitCode::SUCCESS)
}
