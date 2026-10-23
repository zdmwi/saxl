//! IR snapshots for every valid fixture and for Snip.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use saxl_core::FileId;

fn parse_clean(src: &str) -> saxl_core::Model {
    let parsed = saxl_parser::parse(src, FileId(0));
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    parsed.model
}

#[test]
fn valid_fixtures() {
    insta::glob!("../../../tests/fixtures/valid", "*.saxl", |path| {
        let src = std::fs::read_to_string(path).unwrap();
        insta::assert_json_snapshot!(parse_clean(&src));
    });
}

#[test]
fn snip() {
    insta::assert_json_snapshot!(parse_clean(include_str!("../../../examples/snip.saxl")));
}

#[test]
fn quill() {
    insta::assert_json_snapshot!(parse_clean(include_str!(
        "../../../tests/fixtures/library/quill.saxl"
    )));
}
