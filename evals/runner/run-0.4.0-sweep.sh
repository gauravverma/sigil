#!/usr/bin/env bash
# One-shot eval sweep for 0.4.0 compact JSON.
# Run once the API key is exported:
#   export ANTHROPIC_API_KEY=sk-ant-...
#   bash evals/runner/run-0.4.0-sweep.sh
#
# Does:
#   1. Haiku sanity pass (20 runs, ~$0.60)
#   2. Sonnet N=3 published pass (54 runs on 9 tasks, ~$1.70)
#   3. Grades both
#   4. Prints per-task ratio table + aggregate delta vs total cost

set -euo pipefail

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
  echo "ERROR: export ANTHROPIC_API_KEY=... first" >&2
  exit 1
fi

cd "$(dirname "$0")/../.."

DATE=$(date -u +%Y-%m-%d)
HAIKU_DIR="evals/results/$DATE/haiku-4-5/E2"
SONNET_DIR="evals/results/$DATE/sonnet-4-6/E2"

echo "==================================="
echo "1/3: Haiku sanity sweep (20 runs)"
echo "==================================="
python3 evals/runner/run.py --sweep --seeds 1 \
  --model claude-haiku-4-5-20251001 --max-turns 20

echo
echo "==================================="
echo "Grading Haiku sweep..."
echo "==================================="
python3 evals/runner/grade.py "$HAIKU_DIR"

echo
echo "==================================="
echo "2/3: Sonnet N=3 published sweep (54 runs)"
echo "==================================="
python3 evals/runner/run.py --sweep --seeds 3 \
  --model claude-sonnet-4-6 --max-turns 20

echo
echo "==================================="
echo "Grading Sonnet sweep..."
echo "==================================="
python3 evals/runner/grade.py "$SONNET_DIR"

echo
echo "==================================="
echo "3/3: Per-task ratio + aggregate delta"
echo "==================================="
python3 - <<PY
import json, glob, statistics
rows = [json.load(open(f)) for f in glob.glob('$SONNET_DIR/*.json')]
by = {}
for r in rows: by.setdefault(r['task_id'], {}).setdefault(r['arm'], []).append(r)

print('Task     ctrl_in   treat_in   ratio       ctrl_out   treat_out')
print('------   -------   --------   ---------   --------   ---------')
for tid in sorted(by):
    c = by[tid].get('control', []); t = by[tid].get('treatment', [])
    if not (c and t): continue
    ci = statistics.median(r['tokens_in'] for r in c)
    ti = statistics.median(r['tokens_in'] for r in t)
    co = statistics.median(r['tokens_out'] for r in c)
    to = statistics.median(r['tokens_out'] for r in t)
    ratio = ci / ti if ti else 0
    mark = 'win' if ratio > 1.1 else ('LOSS' if ratio < 0.9 else 'tie')
    print(f'{tid}  {ci:8.0f}  {ti:9.0f}   {ratio:5.2f}x {mark:4}  {co:9.0f}   {to:9.0f}')

ci = sum(r['tokens_in'] for r in rows if r['arm']=='control')
ti = sum(r['tokens_in'] for r in rows if r['arm']=='treatment')
co = sum(r['tokens_out'] for r in rows if r['arm']=='control')
to = sum(r['tokens_out'] for r in rows if r['arm']=='treatment')
cost = (ci + ti) * 3 / 1_000_000 + (co + to) * 15 / 1_000_000
print()
print(f'Sum in:  ctrl={ci:>7,}  treat={ti:>7,}  delta={100*(ti-ci)/ci:+.1f}%')
print(f'Sum out: ctrl={co:>7,}  treat={to:>7,}  delta={100*(to-co)/co:+.1f}%')
print(f'Sonnet sweep cost: \${cost:.2f}')
PY

echo
echo "Done. Results: $SONNET_DIR"
