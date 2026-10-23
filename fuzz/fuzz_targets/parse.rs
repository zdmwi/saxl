#![no_main]

use libfuzzer_sys::fuzz_target;
use saxl_core::FileId;

fuzz_target!(|src: &str| {
    let _ = saxl_parser::parse(src, FileId(0));
});
