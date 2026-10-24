#!/bin/sh
# Seeds each fuzz target's corpus from the fixtures, the examples, and files
# generated from the grammar, which cover every construct.
set -eu
cd "$(dirname "$0")"
for target in parse parse_validate fmt_roundtrip; do
  mkdir -p "corpus/$target"
  find ../tests/fixtures ../examples -name '*.saxl' -exec cp {} "corpus/$target/" \;
  (cd .. && cargo xtask gen-samples --out "fuzz/corpus/$target" --count 500 --seed 0)
  rm "corpus/$target/report.json"
done
