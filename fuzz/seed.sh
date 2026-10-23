#!/bin/sh
# Seeds each fuzz target's corpus from the fixtures and examples.
set -eu
cd "$(dirname "$0")"
for target in parse parse_validate; do
  mkdir -p "corpus/$target"
  find ../tests/fixtures ../examples -name '*.saxl' -exec cp {} "corpus/$target/" \;
done
