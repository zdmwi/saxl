//! End-to-end tests of the `saxl` binary: exit codes, output formats, and
//! human output snapshots for every invalid fixture.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `saxl` run from the workspace root, so paths in output are stable.
fn saxl() -> Command {
    let mut cmd = Command::cargo_bin("saxl").unwrap();
    cmd.current_dir(root()).env("NO_COLOR", "1");
    cmd
}

fn stdout(cmd: &mut Command) -> String {
    String::from_utf8(cmd.output().unwrap().stdout).unwrap()
}

const E006: &str = "tests/fixtures/invalid/validate/e006_step_relation.saxl";
const W102: &str = "tests/fixtures/invalid/validate/w102_single_region.saxl";

#[test]
fn check_snip_is_clean() {
    saxl()
        .args(["check", "examples/snip.saxl", "--deny", "warnings"])
        .assert()
        .success()
        .stdout("checked 1 file: 0 errors, 0 warnings, 0 infos\n");
}

#[test]
fn warnings_pass_unless_denied() {
    saxl().args(["check", W102]).assert().code(0);
    saxl()
        .args(["check", W102, "--deny", "warnings"])
        .assert()
        .code(1);
}

#[test]
fn errors_fail() {
    saxl().args(["check", E006]).assert().code(1);
}

#[test]
fn usage_and_io_errors_exit_2() {
    saxl().args(["check", "missing.saxl"]).assert().code(2);
    saxl().args(["check"]).assert().code(2);
    saxl()
        .args(["check", E006, "--format", "xml"])
        .assert()
        .code(2);
    saxl().args(["frobnicate"]).assert().code(2);
}

#[test]
fn check_json() {
    let out = stdout(saxl().args(["check", "--format", "json", E006]));
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    let diags = json.as_array().unwrap();
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(d["file"], E006);
    assert_eq!(d["code"], "E006");
    assert_eq!(d["severity"], "error");
    let label = &d["labels"][0];
    assert_eq!(label["primary"], true);
    assert_eq!(label["range"]["start"]["line"], 9);
    assert_eq!(label["range"]["start"]["col"], 3);
    assert!(label["span"]["start"].is_u64());
    assert!(d["help"].is_string());
}

#[test]
fn check_github() {
    let out = stdout(saxl().args(["check", "--format", "github", E006, W102]));
    let lines: Vec<_> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with(&format!("::error file={E006},line=9,col=3,title=E006::")));
    assert!(lines[1].starts_with(&format!("::warning file={W102},line=6,col=15,title=W102::")));
}

#[test]
fn ir_prints_model() {
    let out = stdout(saxl().args(["ir", "examples/snip.saxl"]));
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(json["ir_version"], 1);
    assert_eq!(json["elements"].as_array().unwrap().len(), 11);
}

#[test]
fn ir_refuses_parse_errors() {
    saxl()
        .args([
            "ir",
            "tests/fixtures/invalid/parse/p002_unterminated_string.saxl",
        ])
        .assert()
        .code(1)
        .stdout("");
}

#[test]
fn ir_schema() {
    let out = stdout(saxl().args(["ir", "--schema"]));
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(json["title"], "Model");
    saxl().args(["ir"]).assert().code(2);
}

#[test]
fn explain() {
    let list = stdout(saxl().arg("explain"));
    assert_eq!(list.lines().count(), 27);
    assert!(list.contains("E006  a scenario step has no matching relation"));
    let doc = stdout(saxl().args(["explain", "w102"]));
    assert!(doc.starts_with("# W102:"));
    saxl().args(["explain", "X999"]).assert().code(2);
}

#[test]
fn init_writes_once() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("saxl-init-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cmd = Command::cargo_bin("saxl").unwrap();
    cmd.current_dir(&dir).arg("init").assert().success();
    let written = std::fs::read_to_string(dir.join("architecture.saxl")).unwrap();
    assert!(written.contains("system Snip"));
    let mut again = Command::cargo_bin("saxl").unwrap();
    again.current_dir(&dir).arg("init").assert().code(2);
    let mut check = Command::cargo_bin("saxl").unwrap();
    check
        .current_dir(&dir)
        .args(["check", "architecture.saxl", "--deny", "warnings"])
        .assert()
        .success();
}

/// Human output for every invalid fixture, and the exit code: 1 when the
/// fixture expects a parse or validation error, 0 otherwise.
#[test]
fn invalid_fixture_output() {
    insta::glob!("../../../tests/fixtures/invalid", "*/*.saxl", |path| {
        let rel = path.strip_prefix(root().canonicalize().unwrap()).unwrap();
        let rel = rel.to_str().unwrap();
        let src = std::fs::read_to_string(path).unwrap();
        let expects_error = src
            .lines()
            .filter_map(|l| l.strip_prefix("# expect: "))
            .any(|code| code.starts_with('P') || code.starts_with('E'));
        let output = saxl().args(["check", rel]).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(i32::from(expects_error)),
            "{rel}"
        );
        insta::assert_snapshot!(String::from_utf8(output.stdout).unwrap());
    });
}

fn out_path(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("saxl-render-test");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn render_formats() {
    let svg = out_path("containers.svg");
    saxl()
        .args(["render", "examples/snip.saxl", "--view", "containers", "-o"])
        .arg(&svg)
        .assert()
        .success();
    assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<svg"));

    let html = out_path("snip.html");
    saxl()
        .args(["render", "examples/snip.saxl", "-o"])
        .arg(&html)
        .assert()
        .success();
    let page = std::fs::read_to_string(&html).unwrap();
    assert!(page.contains("saxlViewer.mount"));

    let json = out_path("all.json");
    saxl()
        .args(["render", "examples/snip.saxl", "-o"])
        .arg(&json)
        .assert()
        .success();
    let scenes: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(scenes.as_array().unwrap().len(), 6);

    let runtime = out_path("hit.json");
    saxl()
        .args([
            "render",
            "examples/snip.saxl",
            "--view",
            "runtime",
            "--scenario",
            "redirect_hit",
            "-o",
        ])
        .arg(&runtime)
        .assert()
        .success();
    let scene: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&runtime).unwrap()).unwrap();
    assert_eq!(scene["view"]["scenario"], "redirect_hit");
}

#[test]
fn render_refuses_bad_input() {
    let out = out_path("bad.svg");
    // Errors in the model: exit 1, nothing written.
    saxl()
        .args(["render", E006, "--view", "context", "-o"])
        .arg(&out)
        .assert()
        .code(1);
    assert!(!out.exists());
    // Usage problems: exit 2.
    for args in [
        vec!["--view", "runtime"],
        vec!["--view", "runtime", "--scenario", "nope"],
        vec!["--view", "context", "--scenario", "redirect_hit"],
        vec![],
    ] {
        saxl()
            .args(["render", "examples/snip.saxl"])
            .args(&args)
            .arg("-o")
            .arg(&out)
            .assert()
            .code(2);
    }
    saxl()
        .args(["render", "examples/snip.saxl", "-o", "out.png"])
        .assert()
        .code(2);
}
