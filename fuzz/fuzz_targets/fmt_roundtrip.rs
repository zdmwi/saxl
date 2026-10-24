#![no_main]

use libfuzzer_sys::fuzz_target;
use saxl_core::FileId;
use serde_json::Value;

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

// Formatting an input that parses must parse again, keep the model and every
// comment, and be idempotent.
fuzz_target!(|src: &str| {
    let Ok(once) = saxl_parser::format(src) else {
        return;
    };
    let twice = saxl_parser::format(&once).expect("formatted output does not parse");
    assert_eq!(once, twice, "formatting is not idempotent");
    assert_eq!(shape(src), shape(&once), "formatting changed the model");
    assert_eq!(comments(src), comments(&once), "formatting changed the comments");
});
