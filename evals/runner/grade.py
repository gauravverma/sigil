"""
Grader for E2 navigation results.

Set-match: case-insensitive, order-insensitive equality against the task's
`expected` list. Tasks with empty `expected` are marked `uncaptured` and
excluded from the summary.

Usage:
  python evals/runner/grade.py evals/results/<date>/E2
"""

from __future__ import annotations

import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parent))
from run import parse_answer  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
TASKS_ROOT = REPO_ROOT / "evals" / "tasks" / "E2_navigation"


def load_expected() -> dict[str, list | None]:
    out: dict[str, list | None] = {}
    for t in TASKS_ROOT.glob("*.yaml"):
        spec = yaml.safe_load(t.read_text())
        exp = spec.get("grader", {}).get("expected")
        out[spec["id"]] = exp if exp else None
    return out


def _norm(x) -> str:
    # Case-insensitive, strip ./ prefix, strip trailing whitespace.
    s = str(x).strip().lower()
    while s.startswith("./"):
        s = s[2:]
    return s


def set_match(actual: list | None, expected: list | None) -> str:
    if expected is None:
        return "uncaptured"
    if actual is None:
        return "no_answer"
    a = {_norm(x) for x in actual}
    e = {_norm(x) for x in expected}
    return "pass" if a == e else "fail"


def main():
    if len(sys.argv) != 2:
        print("usage: grade.py <results_dir>", file=sys.stderr)
        sys.exit(1)
    results_dir = Path(sys.argv[1])
    expected_by_task = load_expected()

    rows: list[dict] = []
    for f in sorted(results_dir.glob("*.json")):
        r = json.loads(f.read_text())
        # Always re-parse from final_text so fixes to the parser rescue old results.
        answer = parse_answer(r.get("final_text"))
        verdict = set_match(answer, expected_by_task.get(r["task_id"]))
        rows.append({**r, "parsed_answer": answer, "verdict": verdict})

    by_arm: dict[str, list[dict]] = defaultdict(list)
    for r in rows:
        by_arm[r["arm"]].append(r)

    print(f"# E2 results — {results_dir}\n")
    print(f"| Arm | Runs | Pass | Fail | Uncaptured | No-answer | Median tokens_in | Median tokens_out |")
    print(f"|---|---:|---:|---:|---:|---:|---:|---:|")
    for arm, rs in sorted(by_arm.items()):
        counts = defaultdict(int)
        for r in rs:
            counts[r["verdict"]] += 1
        ti = statistics.median(r["tokens_in"] for r in rs)
        to = statistics.median(r["tokens_out"] for r in rs)
        print(f"| {arm} | {len(rs)} | {counts['pass']} | {counts['fail']} | {counts['uncaptured']} | {counts['no_answer']} | {ti:.0f} | {to:.0f} |")

    # Per-task view (only tasks with an answer key).
    print("\n## Per-task, graded only\n")
    print("| Task | Arm | Seed | Verdict | tokens_in | tokens_out | turns |")
    print("|---|---|---:|---|---:|---:|---:|")
    for r in rows:
        if r["verdict"] == "uncaptured":
            continue
        print(f"| {r['task_id']} | {r['arm']} | {r['seed']} | {r['verdict']} | {r['tokens_in']} | {r['tokens_out']} | {r['turns']} |")

    # Summary file
    summary = results_dir / "summary.md"
    with summary.open("w") as f:
        sys.stdout = f  # rewrite summary to file too
        print(f"# E2 results — {results_dir.name}\n")
        # Simplified: just the aggregate table
        print(f"| Arm | Runs | Pass | Fail | Uncaptured | Median tokens_in | Median tokens_out |")
        print(f"|---|---:|---:|---:|---:|---:|---:|")
        for arm, rs in sorted(by_arm.items()):
            counts = defaultdict(int)
            for r in rs:
                counts[r["verdict"]] += 1
            ti = statistics.median(r["tokens_in"] for r in rs)
            to = statistics.median(r["tokens_out"] for r in rs)
            print(f"| {arm} | {len(rs)} | {counts['pass']} | {counts['fail']} | {counts['uncaptured']} | {ti:.0f} | {to:.0f} |")


if __name__ == "__main__":
    main()
