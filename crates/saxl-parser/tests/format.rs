//! Formatter properties on every saxl source in the repository that parses:
//! formatting is idempotent, keeps the model, and keeps every comment.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};

use saxl_core::FileId;
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.saxl` file and every `saxl` block in the rule docs, with a name.
fn sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in [
        "examples",
        "tests/fixtures/valid",
        "tests/fixtures/library",
        "tests/fixtures/invalid/parse",
        "tests/fixtures/invalid/validate",
    ] {
        for entry in std::fs::read_dir(root().join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "saxl") {
                out.push((
                    path.display().to_string(),
                    std::fs::read_to_string(&path).unwrap(),
                ));
            }
        }
    }
    for entry in std::fs::read_dir(root().join("crates/saxl-validator/rules")).unwrap() {
        let path = entry.unwrap().path();
        let md = std::fs::read_to_string(&path).unwrap();
        for (i, block) in md.split("```saxl\n").skip(1).enumerate() {
            let code = block.split("```").next().unwrap_or_default();
            out.push((format!("{}#{i}", path.display()), code.to_owned()));
        }
    }
    out.sort();
    out
}

/// The model as JSON with every span removed, so models compare by content.
fn shape(src: &str) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(map) => {
                map.remove("span");
                map.remove("code_span");
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(saxl_parser::parse(src, FileId(0)).model).unwrap();
    strip(&mut v);
    v
}

fn comments(src: &str) -> Vec<String> {
    let lexed = saxl_parser::lexer::lex(src, FileId(0));
    lexed
        .comments
        .iter()
        .map(|c| src[c.start as usize..c.end as usize].trim_end().to_owned())
        .collect()
}

#[test]
fn formatting_is_idempotent_and_lossless() {
    let mut checked = 0;
    for (name, src) in sources() {
        let Ok(once) = saxl_parser::format(&src) else {
            continue;
        };
        checked += 1;
        let twice = saxl_parser::format(&once)
            .unwrap_or_else(|e| panic!("{name}: formatted output does not parse: {e:?}"));
        assert_eq!(once, twice, "{name}: not idempotent");
        assert_eq!(shape(&src), shape(&once), "{name}: model changed");
        assert_eq!(comments(&src), comments(&once), "{name}: comments changed");
    }
    // Every fixture except the parse-error ones, plus both examples per rule doc.
    assert!(checked > 70, "only {checked} sources formatted");
}

#[test]
fn snip_is_already_formatted() {
    let src = std::fs::read_to_string(root().join("examples/snip.saxl")).unwrap();
    assert_eq!(saxl_parser::format(&src).unwrap(), src);
}
