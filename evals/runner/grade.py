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
    if len(sys.argv) < 2:
        print("usage: grade.py <results_dir> [--baseline <dir>]", file=sys.stderr)
        sys.exit(1)
    results_dir = Path(sys.argv[1])
    baseline_dir: Path | None = None
    if len(sys.argv) >= 4 and sys.argv[2] == "--baseline":
        baseline_dir = Path(sys.argv[3])
    expected_by_task = load_expected()

    # Auto-detect baseline: if the caller didn't pass one but
    # evals/results/baselines/<model>/<task-set>/ exists, use that.
    # Convention: the baseline dir mirrors the results dir shape,
    # e.g. evals/results/baselines/sonnet-4-6/E2/.
    if baseline_dir is None:
        parts = results_dir.parts
        if "results" in parts:
            auto = (
                REPO_ROOT
                / "evals"
                / "results"
                / "baselines"
                / "/".join(parts[parts.index("results") + 2:])
            )
            if auto.exists() and auto != results_dir:
                baseline_dir = auto

    rows: list[dict] = []
    seen_keys: set[tuple] = set()
    for f in sorted(results_dir.glob("*.json")):
        if f.name == "summary.md":
            continue
        r = json.loads(f.read_text())
        seen_keys.add((r["task_id"], r["arm"], r["seed"]))
        answer = parse_answer(r.get("final_text"))
        spec = expected_by_task.get(r["task_id"], {})
        gtype = spec.get("type", "set_match")
        exp = spec.get("expected")
        verdict = fact_match(answer, exp) if gtype == "fact_match" else set_match(answer, exp)
        rows.append({**r, "parsed_answer": answer, "verdict": verdict, "source": "run"})

    # Fallback: pull rows from baseline_dir for keys not in the current run.
    # Lets `--arm treatment` sweeps compare against a frozen control without
    # paying to re-run it.
    if baseline_dir:
        for f in sorted(baseline_dir.glob("*.json")):
            if f.name == "summary.md":
                continue
            r = json.loads(f.read_text())
            key = (r["task_id"], r["arm"], r["seed"])
            if key in seen_keys:
                continue
            answer = parse_answer(r.get("final_text"))
            spec = expected_by_task.get(r["task_id"], {})
            gtype = spec.get("type", "set_match")
            exp = spec.get("expected")
            verdict = fact_match(answer, exp) if gtype == "fact_match" else set_match(answer, exp)
            rows.append({**r, "parsed_answer": answer, "verdict": verdict, "source": "baseline"})
        n_baseline = sum(1 for r in rows if r["source"] == "baseline")
        if n_baseline:
            print(f"(pulled {n_baseline} rows from baseline: {baseline_dir})", file=sys.stderr)

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
