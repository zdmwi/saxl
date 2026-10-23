//! A deterministic mutation test: mangled fixtures must never panic the parser
//! or the validator. Complements the cargo-fuzz targets in `fuzz/`.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::path::Path;

use saxl_core::FileId;

/// xorshift64*, so the test is reproducible without a dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const PIECES: &[&str] = &[
    "\n",
    " ",
    "{",
    "}",
    ",",
    "+",
    "->",
    "~>",
    "-->",
    ":",
    "\"",
    "#",
    "x0",
    "x3",
    "parallel {\n",
    "}\n",
    "scenario s {\n",
    "deploy region r {\n",
    "cluster c {\n",
    "managed {\n",
    "cdn E\n",
    "allow W102 \"r\"\n",
    "allow E001 \"r\"\n",
    "system S\n",
    "service A\n",
    "store A\n",
    "queue Q\n",
    "actor U\n",
    "A -> A\n",
    "Q ~> A\n",
    "A --> U\n",
    "U -> E -> A\n",
    "data D in A { f }\n",
    "data D via Q -> A { f }\n",
    "→",
    "é",
    "@",
];

fn mutate(src: &str, rng: &mut Rng) -> String {
    let mut s = src.to_owned();
    for _ in 0..1 + rng.below(4) {
        let mut at = rng.below(s.len() + 1);
        while !s.is_char_boundary(at) {
            at -= 1;
        }
        match rng.below(3) {
            0 => s.insert_str(at, PIECES[rng.below(PIECES.len())]),
            1 => {
                let mut end = (at + 1 + rng.below(12)).min(s.len());
                while !s.is_char_boundary(end) {
                    end -= 1;
                }
                s.replace_range(at..end, "");
            }
            _ => s.truncate(at),
        }
    }
    s
}

#[test]
fn mutated_fixtures_never_panic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for dir in [
        "tests/fixtures/valid",
        "tests/fixtures/invalid/parse",
        "tests/fixtures/invalid/validate",
        "examples",
    ] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            sources.push(std::fs::read_to_string(entry.unwrap().path()).unwrap());
        }
    }
    let mut rng = Rng(0x5A71_5A71_5A71_5A71);
    for src in &sources {
        for _ in 0..300 {
            let input = mutate(src, &mut rng);
            let parsed = saxl_parser::parse(&input, FileId(0));
            // Validate even with parse errors: the validator must cope with any model.
            let _ = saxl_validator::validate(parsed.model);
        }
    }
}
