//! Build helpers for the saxl workspace. Run with `cargo xtask <command>`.

use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo xtask <types|viewer|wasm|playground|ci>";

fn main() -> ExitCode {
    let Some(cmd) = std::env::args().nth(1) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let result = match cmd.as_str() {
        "ci" => ci(),
        "types" | "viewer" | "wasm" | "playground" => {
            Err(format!("`{cmd}` is not implemented yet"))
        }
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the checks of the CI `rust` and `self-check` jobs.
fn ci() -> Result<(), String> {
    run("cargo", &["fmt", "--all", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["test", "--workspace"])?;
    Ok(())
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    eprintln!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed", args.join(" ")))
    }
}
