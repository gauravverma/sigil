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
TASKS_ROOT = REPO_ROOT / "evals" / "tasks"


def load_expected() -> dict[str, dict]:
    """Load (grader_type, expected) per task across every subdirectory of
    `evals/tasks/`. Each task YAML carries `grader.type` and
    `grader.expected`. Tasks without expected values grade as
    `uncaptured`.
    """
    out: dict[str, dict] = {}
    for t in TASKS_ROOT.rglob("*.yaml"):
        if "_parked" in t.parts:
            continue
        spec = yaml.safe_load(t.read_text())
        grader = spec.get("grader", {})
        out[spec["id"]] = {
            "type": grader.get("type", "set_match"),
            "expected": grader.get("expected"),
        }
    return out


def _norm(x) -> str:
    # Case-insensitive, strip ./ prefix, strip trailing whitespace.
    s = str(x).strip().lower()
    while s.startswith("./"):
        s = s[2:]
    return s


def set_match(actual, expected) -> str:
    if expected is None or not expected:
        return "uncaptured"
    if actual is None:
        return "no_answer"
    if not isinstance(actual, list):
        return "fail"
    a = {_norm(x) for x in actual}
    e = {_norm(x) for x in expected}
    return "pass" if a == e else "fail"


def fact_match(actual, expected) -> str:
    """Dict equality with per-value normalization. Used by SWE-bench-like
    tasks where the answer is a labeled tuple (e.g. {method, file, class}).
    Extra keys in `actual` are ignored so agents may decorate answers.

    Also tolerates a single-element array wrapping — models occasionally
    reply with `[{...}]` when the system prompt mentioned "array" but the
    task asked for an object. The answer is semantically correct; unwrap
    before comparison.
    """
    if expected is None or not expected:
        return "uncaptured"
    if actual is None:
        return "no_answer"
    if isinstance(actual, list) and len(actual) == 1 and isinstance(actual[0], dict):
        actual = actual[0]
    if not isinstance(actual, dict):
        return "fail"
    for k, v in expected.items():
        if k not in actual:
            return "fail"
        if _norm(actual[k]) != _norm(v):
            return "fail"
    return "pass"


def main():
    if len(sys.argv) != 2:
        print("usage: grade.py <results_dir>", file=sys.stderr)
        sys.exit(1)
    results_dir = Path(sys.argv[1])
    expected_by_task = load_expected()

    rows: list[dict] = []
    for f in sorted(results_dir.glob("*.json")):
        if f.name == "summary.md":
            continue
        r = json.loads(f.read_text())
        # Re-parse from final_text so parser fixes rescue old results.
        answer = parse_answer(r.get("final_text"))
        spec = expected_by_task.get(r["task_id"], {})
        gtype = spec.get("type", "set_match")
        exp = spec.get("expected")
        if gtype == "fact_match":
            verdict = fact_match(answer, exp)
        else:
            verdict = set_match(answer, exp)
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
