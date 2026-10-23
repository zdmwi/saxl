//! Build helpers for the saxl workspace. Run with `cargo xtask <command>`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo xtask <types|viewer|wasm|playground|ci>";

/// Where ts-rs writes the generated TypeScript types.
const GENERATED: &str = "crates/saxl-renderer/viewer/src/generated";

fn main() -> ExitCode {
    let Some(cmd) = std::env::args().nth(1) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let result = match cmd.as_str() {
        "types" => types(),
        "viewer" => viewer(),
        "wasm" => wasm(),
        "playground" => playground(),
        "ci" => ci(),
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

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Regenerates the TypeScript types for scenes and diagnostics.
fn types() -> Result<(), String> {
    let dir = root().join(GENERATED);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("can't clear {GENERATED}: {e}"))?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("can't create {GENERATED}: {e}"))?;
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("can't resolve {GENERATED}: {e}"))?;
    run_with_env(
        "cargo",
        &[
            "test",
            "-p",
            "saxl-core",
            "-p",
            "saxl-renderer",
            "--features",
            "saxl-core/ts,saxl-renderer/ts",
            "export_bindings",
        ],
        &[("TS_RS_EXPORT_DIR", dir.as_os_str())],
    )
}

/// Bundles the viewer into `crates/saxl-renderer/assets/viewer.js`.
fn viewer() -> Result<(), String> {
    run("npm", &["ci"])?;
    run("npm", &["run", "build", "-w", "@saxl/viewer"])
}

/// The `wasm-bindgen-cli` version, which must equal the `wasm-bindgen` crate's.
const WASM_BINDGEN: &str = "0.2.129";

/// Builds `saxl-wasm` and generates its JS bindings in `playground/web/src/wasm`.
fn wasm() -> Result<(), String> {
    let installed = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    if installed.as_deref() != Some(&format!("wasm-bindgen {WASM_BINDGEN}")) {
        return Err(format!(
            "wasm-bindgen-cli {WASM_BINDGEN} is required (found {}); install it with:\n  \
             cargo install wasm-bindgen-cli --version {WASM_BINDGEN} --locked",
            installed.as_deref().unwrap_or("none")
        ));
    }
    run(
        "cargo",
        &[
            "build",
            "-p",
            "saxl-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ],
    )?;
    let target = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_owned());
    let wasm = format!("{target}/wasm32-unknown-unknown/release/saxl_wasm.wasm");
    run(
        "wasm-bindgen",
        &[
            "--target",
            "web",
            "--out-dir",
            "playground/web/src/wasm",
            &wasm,
        ],
    )
}

/// Builds the wasm bindings, then the playground into `playground/web/dist`.
fn playground() -> Result<(), String> {
    wasm()?;
    run("npm", &["ci"])?;
    run("npm", &["run", "build", "-w", "saxl-playground"])
}

/// Runs what CI runs: the `rust`, `self-check`, and `generated` jobs.
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
    run(
        "cargo",
        &[
            "run",
            "-p",
            "saxl-cli",
            "--",
            "check",
            "examples/snip.saxl",
            "--deny",
            "warnings",
        ],
    )?;
    types()?;
    viewer()?;
    run("npm", &["run", "typecheck", "-w", "@saxl/viewer"])?;
    run(
        "git",
        &[
            "diff",
            "--exit-code",
            "--",
            GENERATED,
            "crates/saxl-renderer/assets/viewer.js",
        ],
    )
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    run_with_env(program, args, &[])
}

fn run_with_env(
    program: &str,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
) -> Result<(), String> {
    eprintln!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(root())
        .status()
        .map_err(|e| format!("failed to run {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed", args.join(" ")))
    }
}
