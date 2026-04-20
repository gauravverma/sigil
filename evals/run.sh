#!/bin/sh
# Capture a token-reduction benchmark snapshot to evals/results/.
# Usage: evals/run.sh [refspec]   (default HEAD~3..HEAD)

set -eu

REFSPEC="${1:-HEAD~3..HEAD}"
# sigil reports its version via --version; strip the leading name.
VERSION="$(cargo run --quiet -- --version 2>/dev/null | awk '{print $NF}')"
SAFE_REF="$(printf '%s' "$REFSPEC" | tr '/.~^:' '-----')"
OUT="evals/results/${VERSION}-${SAFE_REF}.json"

# The benchmark assumes a current index. Re-run `sigil index` first if in doubt.
cargo run --quiet -- benchmark --refspec "$REFSPEC" --format json --pretty > "$OUT"

# Human-readable summary to stderr.
python3 - "$OUT" <<'PY' >&2
import json, sys
d = json.load(open(sys.argv[1]))
print(f"sigil {d['sigil_version']} · refspec {d['refspec']} · median {d['median_ratio']:.2f}x")
for q in d['queries']:
    print(f"  {q['name']:30}  {q['control_tokens']:>7} -> {q['treatment_tokens']:>6}  ({q['ratio']:.2f}x)")
print(f"wrote {sys.argv[1]}")
PY
