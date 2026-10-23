#![no_main]

use libfuzzer_sys::fuzz_target;
use saxl_core::FileId;

fuzz_target!(|src: &str| {
    let parsed = saxl_parser::parse(src, FileId(0));
    if parsed.diagnostics.is_empty() {
        let _ = saxl_validator::validate(parsed.model);
    }
});
