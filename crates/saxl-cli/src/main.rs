//! The `saxl` command-line tool.

mod check;
mod explain;
mod init;
mod ir;
mod load;
mod render;
mod report;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Exit code for usage errors, I/O errors, and unknown names.
const EXIT_USAGE: u8 = 2;

/// A failure that is not about the input's content. Exits with code 2.
#[derive(Debug)]
pub struct CliError(pub String);

#[derive(Parser)]
#[command(
    name = "saxl",
    version,
    about = "Describe a software architecture once, check it, and render every view"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and validate files.
    Check(check::Args),
    /// Render a view as HTML, SVG, or JSON.
    Render(render::Args),
    /// Print the IR as JSON, or its JSON Schema.
    Ir(ir::Args),
    /// Explain a diagnostic code, or list every code.
    Explain(explain::Args),
    /// Write a starter file.
    Init(init::Args),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Check(args) => check::run(&args),
        Command::Render(args) => render::run(&args),
        Command::Ir(args) => ir::run(&args),
        Command::Explain(args) => explain::run(&args),
        Command::Init(args) => init::run(&args),
    };
    result.unwrap_or_else(|CliError(message)| {
        eprintln!("error: {message}");
        ExitCode::from(EXIT_USAGE)
    })
}

/// Whether to color terminal output written to stdout.
fn color_stdout() -> bool {
    use std::io::IsTerminal;
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

/// Whether to color terminal output written to stderr.
fn color_stderr() -> bool {
    use std::io::IsTerminal;
    std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal()
}
