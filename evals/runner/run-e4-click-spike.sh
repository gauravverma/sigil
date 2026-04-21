#!/usr/bin/env bash
# One-shot spike: measure token usage on one SWE-bench-Lite-style task.
#
# Target: pallets/click at 04ef3a6 — "find the Parameter.get_default method
# that resolves default values" (code-localization task, SWE-bench phase 1
# style: locate the relevant code without writing a patch).
#
# Usage:
#   export ANTHROPIC_API_KEY=sk-ant-...
#   bash evals/runner/run-e4-click-spike.sh
#
# Cost estimate: ~$1–2 total (Haiku smoke + Sonnet N=3).

set -euo pipefail

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
  echo "ERROR: export ANTHROPIC_API_KEY=... first" >&2
  exit 1
fi

cd "$(dirname "$0")/../.."

DATE=$(date -u +%Y-%m-%d)

echo "==================================="
echo "1/3: Haiku smoke (2 runs, ~\$0.20)"
echo "==================================="
python3 evals/runner/run.py \
  --task-set E4_swebench_like --sweep --seeds 1 \
  --model claude-haiku-4-5-20251001 --max-turns 20 --workers 8

echo
echo "==================================="
echo "2/3: Sonnet N=3 (6 runs, ~\$1-2)"
echo "==================================="
python3 evals/runner/run.py \
  --task-set E4_swebench_like --sweep --seeds 3 \
  --model claude-sonnet-4-6 --max-turns 30 --workers 8

echo
echo "==================================="
echo "3/3: Grade + per-arm token summary"
echo "==================================="
python3 evals/runner/grade.py "evals/results/$DATE/sonnet-4-6/E4"
echo
python3 - <<PY
import json, glob, statistics

for label, path in [
    ("Haiku N=1", "evals/results/$DATE/haiku-4-5/E4/*.json"),
    ("Sonnet N=3", "evals/results/$DATE/sonnet-4-6/E4/*.json"),
]:
    rows = [json.load(open(f)) for f in glob.glob(path) if 'summary' not in f]
    if not rows: continue
    print(f'\n{label}:')
    print(f'  {"arm":10} {"tokens_in":>10} {"tokens_out":>10} {"turns":>6}')
    for arm in ['control', 'treatment']:
        xs = [r for r in rows if r['arm']==arm]
        if not xs: continue
        med_in = statistics.median(r['tokens_in'] for r in xs)
        med_out = statistics.median(r['tokens_out'] for r in xs)
        med_turns = statistics.median(r['turns'] for r in xs)
        print(f'  {arm:10} {med_in:>10,.0f} {med_out:>10,.0f} {med_turns:>6.0f}')
    c = [r for r in rows if r['arm']=='control']
    t = [r for r in rows if r['arm']=='treatment']
    if c and t:
        ci = statistics.median(r['tokens_in'] for r in c)
        ti = statistics.median(r['tokens_in'] for r in t)
        ratio = ci/ti if ti else 0
        print(f'  control/treatment input ratio: {ratio:.2f}x ({"sigil wins" if ratio>1 else "control wins"})')
    tot_in = sum(r['tokens_in'] for r in rows)
    tot_out = sum(r['tokens_out'] for r in rows)
    cost = tot_in * 3/1e6 + tot_out*15/1e6 if 'sonnet' in label.lower() else tot_in*1/1e6 + tot_out*5/1e6
    print(f'  total cost this arm-set: ~\${cost:.2f}')
PY

echo
echo "Done. Results under: evals/results/$DATE/{haiku-4-5,sonnet-4-6}/E4/"
