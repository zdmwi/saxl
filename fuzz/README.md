# Fuzzing

Three `cargo-fuzz` targets check that no input panics or hangs the toolchain, and that formatting is safe:

| Target | What it runs |
|---|---|
| `parse` | `saxl_parser::parse` |
| `parse_validate` | `parse`, then `saxl_validator::validate` when parsing succeeds |
| `fmt_roundtrip` | `saxl_parser::format`; when the input parses, the output must parse, keep the model (ignoring spans) and every comment, and format to itself |

## Setup

Fuzzing needs a nightly toolchain recent enough for edition 2024, and `cargo-fuzz`:

```sh
rustup toolchain install nightly --profile minimal
cargo install cargo-fuzz --locked
```

## Running

Seed each target's corpus from the fixtures, the examples, and 500 files from `cargo xtask gen-samples`, then run from this directory:

```sh
./seed.sh
cargo +nightly fuzz run parse -- -dict=saxl.dict -max_total_time=3600 -timeout=10
cargo +nightly fuzz run parse_validate -- -dict=saxl.dict -max_total_time=3600 -timeout=10
cargo +nightly fuzz run fmt_roundtrip -- -dict=saxl.dict -max_total_time=3600 -timeout=10
```

`saxl.dict` lists saxl keywords, arrows, and attributes so mutations produce input the parser gets further into. Crashing or slow inputs are written to `artifacts/<target>/`; reproduce one with `cargo +nightly fuzz run <target> <artifact>`. `corpus/` and `artifacts/` are git-ignored.
