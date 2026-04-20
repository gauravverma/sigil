#!/bin/sh
# evals/cross_repo.sh — run `sigil benchmark` across a curated OSS corpus.
#
# Each row in the corpus TSV is: slug \t url \t ref \t refspec
# The script clones each repo into a temp dir, indexes it, runs
# `sigil benchmark --refspec <refspec>`, and records the JSON output at
# evals/results/cross-repo-<date>/<slug>.json. A summary table lands at
# evals/results/cross-repo-<date>/README.md.
#
# Usage:
#   ./evals/cross_repo.sh                         # uses evals/corpus.tsv
#   ./evals/cross_repo.sh path/to/custom.tsv      # custom corpus
#   CORPUS_DIR=/tmp/sigil-corpus ./evals/cross_repo.sh   # persist clones
#
# Why cross-repo matters: the sigil-self benchmark publishes 25.91× median
# reduction, but that's one repo. Agent token-cost savings scale with
# corpus size and coupling density — this script defends the headline by
# showing how numbers shift across a representative set (small lib,
# medium app, large monorepo; Rust / Python / TS / Go).

set -eu

CORPUS_TSV="${1:-evals/corpus.tsv}"
DATE="$(date +%Y-%m-%d)"
OUT_DIR="evals/results/cross-repo-${DATE}"
CORPUS_DIR="${CORPUS_DIR:-$(mktemp -d -t sigil-corpus.XXXXXX)}"
SIGIL_BIN="${SIGIL_BIN:-sigil}"

if [ ! -r "$CORPUS_TSV" ]; then
    echo "corpus file not found: $CORPUS_TSV" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"
echo "cross-repo benchmark:"
echo "  corpus:  $CORPUS_TSV"
echo "  out:     $OUT_DIR"
echo "  clones:  $CORPUS_DIR"
echo

# Discard blank lines and comments (# at start).
grep -vE '^\s*(#|$)' "$CORPUS_TSV" | while IFS="$(printf '\t')" read -r SLUG URL REF REFSPEC; do
    REPO_DIR="$CORPUS_DIR/$SLUG"
    RESULT="$OUT_DIR/$SLUG.json"

    echo "=== $SLUG ==="
    if [ ! -d "$REPO_DIR/.git" ]; then
        git clone --depth 200 "$URL" "$REPO_DIR"
    fi
    (
        cd "$REPO_DIR"
        git fetch --depth 200 origin "$REF" 2>/dev/null || true
        git checkout -q "$REF"
        "$SIGIL_BIN" index --no-rank >/dev/null 2>&1 || true
        "$SIGIL_BIN" index >/dev/null 2>&1
        "$SIGIL_BIN" benchmark --refspec "$REFSPEC" --format json --pretty
    ) > "$RESULT"
    python3 - "$RESULT" "$SLUG" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
print(f"  {sys.argv[2]:20} median {d['median_ratio']:.2f}x")
for q in d['queries']:
    skip = " [control-skipped]" if q.get("control_skipped") else ""
    print(f"    {q['name']:30}  {q['control_tokens']:>7} -> {q['treatment_tokens']:>6}  ({q['ratio']:.2f}x){skip}")
PY
    echo
done

# Emit summary table
python3 - "$OUT_DIR" <<'PY' > "$OUT_DIR/README.md"
import json, os, sys, glob
out_dir = sys.argv[1]
rows = []
for path in sorted(glob.glob(os.path.join(out_dir, "*.json"))):
    slug = os.path.splitext(os.path.basename(path))[0]
    d = json.load(open(path))
    rows.append((slug, d['median_ratio'], d['refspec'], d['sigil_version'], d['queries']))

print("# Cross-repo benchmark snapshot\n")
print(f"Captured {os.path.basename(out_dir)}.\n")
print("| Repo | Sigil version | Refspec | Median reduction |")
print("|---|---|---|---:|")
for slug, median, refspec, version, _ in rows:
    print(f"| `{slug}` | {version} | `{refspec}` | **{median:.2f}x** |")
print("\n## Per-query breakdown\n")
for slug, _, refspec, _, queries in rows:
    print(f"### `{slug}` ({refspec})\n")
    print("| Query | Control tokens | Sigil tokens | Ratio |")
    print("|---|---:|---:|---:|")
    for q in queries:
        ratio = "n/a" if q.get("control_skipped") else f"{q['ratio']:.2f}x"
        print(f"| {q['name']} | {q['control_tokens']} | {q['treatment_tokens']} | {ratio} |")
    print()
PY

echo "wrote $OUT_DIR/README.md"
