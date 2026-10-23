//! The fixture harness. Runs every fixture through the parser and,
//! when parsing succeeds, the validator, and checks the `# expect:` headers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use saxl_core::{Diagnostic, FileId, LineIndex};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn saxl_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "saxl"))
        .collect();
    files.sort();
    files
}

/// Parse, then validate if there are no parse errors.
fn check(src: &str) -> Vec<Diagnostic> {
    let parsed = saxl_parser::parse(src, FileId(0));
    if !parsed.diagnostics.is_empty() {
        return parsed.diagnostics;
    }
    saxl_validator::validate(parsed.model).diagnostics
}

struct Expect {
    code: String,
    at: Option<(u32, u32)>,
}

fn expectations(src: &str) -> Vec<Expect> {
    src.lines()
        .filter_map(|l| l.strip_prefix("# expect:"))
        .map(|rest| {
            let (code, at) = match rest.split_once('@') {
                Some((code, at)) => {
                    let (line, col) = at
                        .trim()
                        .split_once(':')
                        .expect("expect position is line:col");
                    (code, Some((line.parse().unwrap(), col.parse().unwrap())))
                }
                None => (rest, None),
            };
            Expect {
                code: code.trim().to_owned(),
                at,
            }
        })
        .collect()
}

fn describe(src: &str, diags: &[Diagnostic]) -> String {
    let idx = LineIndex::new(src);
    diags
        .iter()
        .map(|d| {
            let p = idx.position(src, d.sort_key());
            format!("  {} @ {}:{}  {}", d.code, p.line, p.col, d.message)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns a failure message, if any.
fn run_fixture(path: &Path) -> Option<String> {
    let src = std::fs::read_to_string(path).unwrap();
    let diags = check(&src);
    let expects = expectations(&src);
    let idx = LineIndex::new(&src);
    let pos = |d: &Diagnostic| {
        let p = idx.position(&src, d.sort_key());
        (p.line, p.col)
    };
    let matches =
        |e: &Expect, d: &Diagnostic| e.code == d.code.0 && e.at.is_none_or(|at| at == pos(d));
    let unexpected = diags.iter().any(|d| !expects.iter().any(|e| matches(e, d)));
    let unmet = expects.iter().any(|e| !diags.iter().any(|d| matches(e, d)));
    (unexpected || unmet).then(|| {
        let wanted: Vec<_> = expects
            .iter()
            .map(|e| match e.at {
                Some((l, c)) => format!("  {} @ {l}:{c}", e.code),
                None => format!("  {}", e.code),
            })
            .collect();
        format!(
            "{}\nexpected:\n{}\nactual:\n{}",
            path.display(),
            wanted.join("\n"),
            describe(&src, &diags)
        )
    })
}

#[test]
fn valid_fixtures_have_no_diagnostics() {
    let root = workspace();
    let mut files = saxl_files(&root.join("tests/fixtures/valid"));
    files.extend(saxl_files(&root.join("examples")));
    assert!(files.len() >= 3);
    let failures: Vec<_> = files
        .iter()
        .filter_map(|path| {
            let src = std::fs::read_to_string(path).unwrap();
            let diags = check(&src);
            (!diags.is_empty()).then(|| format!("{}\n{}", path.display(), describe(&src, &diags)))
        })
        .collect();
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

#[test]
fn invalid_fixtures_match_expectations() {
    let root = workspace().join("tests/fixtures/invalid");
    let mut files = saxl_files(&root.join("parse"));
    files.extend(saxl_files(&root.join("validate")));
    let failures: Vec<_> = files.iter().filter_map(|p| run_fixture(p)).collect();
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

fn all_codes() -> Vec<&'static str> {
    let mut codes: Vec<_> = saxl_validator::PARSE_CODES
        .iter()
        .map(|(c, _)| *c)
        .collect();
    codes.extend(saxl_validator::rules().iter().map(|r| r.code()));
    codes
}

#[test]
fn every_code_has_a_fixture() {
    let root = workspace().join("tests/fixtures/invalid");
    let mut covered = BTreeSet::new();
    for path in saxl_files(&root.join("parse"))
        .into_iter()
        .chain(saxl_files(&root.join("validate")))
    {
        let src = std::fs::read_to_string(path).unwrap();
        covered.extend(expectations(&src).into_iter().map(|e| e.code));
    }
    let missing: Vec<_> = all_codes()
        .into_iter()
        .filter(|c| !covered.contains(*c))
        .collect();
    assert!(missing.is_empty(), "codes without a fixture: {missing:?}");
}

/// The fenced `saxl` blocks of a markdown document.
fn saxl_blocks(md: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in md.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```saxl") => current = Some(String::new()),
            (Some(block), "```") => {
                blocks.push(std::mem::take(block));
                current = None;
            }
            (Some(block), _) => {
                block.push_str(line);
                block.push('\n');
            }
            (None, _) => {}
        }
    }
    blocks
}

#[test]
fn rule_docs_examples() {
    let mut failures = Vec::new();
    for code in all_codes() {
        let Some(doc) = saxl_validator::explain(code) else {
            failures.push(format!("{code}: no documentation"));
            continue;
        };
        let blocks = saxl_blocks(doc);
        let [failing, fixed] = blocks.as_slice() else {
            failures.push(format!(
                "{code}: expected 2 saxl examples, found {}",
                blocks.len()
            ));
            continue;
        };
        let has = |src: &str| check(src).iter().any(|d| d.code.0 == code);
        if !has(failing) {
            failures.push(format!(
                "{code}: failing example does not report {code}:\n{}",
                describe(failing, &check(failing))
            ));
        }
        if has(fixed) {
            failures.push(format!("{code}: fixed example still reports {code}"));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n\n"));
}

#[test]
fn snip_validates() {
    let src = std::fs::read_to_string(workspace().join("examples/snip.saxl")).unwrap();
    let parsed = saxl_parser::parse(&src, FileId(0));
    assert!(parsed.diagnostics.is_empty());
    let validation = saxl_validator::validate(parsed.model);
    assert!(
        validation.diagnostics.is_empty(),
        "{:#?}",
        validation.diagnostics
    );
    assert!(validation.model.is_some());
}

#[test]
fn errors_block_the_validated_model() {
    let parsed = saxl_parser::parse("system S\nactor A\nexternal B\nA -> B\n", FileId(0));
    let validation = saxl_validator::validate(parsed.model);
    assert!(validation.model.is_none());
}
