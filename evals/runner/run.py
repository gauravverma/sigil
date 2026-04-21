"""
Eval runner v0.0.1 — E2 navigation.

Runs one task × arm × seed, or a sweep over all tasks. Records token
counts + the model's final answer to a JSON result file for later
grading.

Usage:
  python evals/runner/run.py --task evals/tasks/E2_navigation/001-*.yaml \
                             --arm treatment --seed 1 \
                             --model claude-haiku-4-5

  python evals/runner/run.py --sweep --model claude-sonnet-4-6 --seeds 3

The runner does NOT grade — that's grade.py's job. It writes a result
JSON per (task, arm, seed) and moves on.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import random
import re
import shlex
import subprocess
import sys
from pathlib import Path
from typing import Any

import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
TASKS_ROOT = REPO_ROOT / "evals" / "tasks"
RESULTS_ROOT = REPO_ROOT / "evals" / "results"


def task_repo_root(task: dict) -> Path:
    """Effective CWD for the agent's tools. Defaults to the sigil repo.
    Tasks targeting external codebases (SWE-bench-like) set `repo:
    evals/_workbench/<slug>` to point at a cloned tree."""
    if "repo" in task and task["repo"]:
        p = REPO_ROOT / task["repo"]
        if not p.exists():
            raise RuntimeError(f"task {task['id']} references missing repo: {p}")
        return p.resolve()
    return REPO_ROOT

# Capability blurb injected into the treatment arm only. Mirrors the text
# the hook installers write into CLAUDE.md etc. — capability-describing,
# not preference-giving.
SIGIL_BLURB = """\
You also have `sigil` available on PATH — a deterministic structural code
intelligence CLI. Capabilities:
  sigil search <name>           symbols by name substring — returns file + line +
                                kind + parent class. Empty result means "no such
                                symbol," not "retry with more keywords."
  sigil symbols <FILE>          every entity in ONE file with its parent class.
                                Prefer this once you've narrowed to a file.
  sigil callers <name>          who calls <name>: returns file + caller + line.
  sigil callees <caller>        what <caller> calls.
  sigil map --tokens N          ranked codebase digest (orientation).
  sigil context <symbol>        signature + callers + callees + related types.

All commands support `--json` for machine-readable output. Prefer short,
specific substrings ("default", not "get_default_from_env") and narrow to
a file with `sigil symbols FILE` as soon as you know which file matters.
"""

SYSTEM_PROMPT_BASE = """\
You are helping answer a navigation question about a code repository at
{repo}. You have tools: read_file, grep, glob, bash. Be efficient — aim
for the minimum number of tool calls that gives you a confident answer.

When you have the answer, reply with ONLY valid JSON as the final
assistant message (no prose, no markdown fences, no extra text). The
shape is specified by the question — it may be a JSON array, a JSON
object, or another JSON value. Match the exact shape requested.
"""

TOOLS = [
    {
        "name": "read_file",
        "description": "Read a file from the repository. Returns up to 2000 lines.",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Repo-relative path"},
                "offset": {"type": "integer", "description": "Optional 1-based line offset", "default": 1},
                "limit": {"type": "integer", "description": "Max lines to return", "default": 2000},
            },
            "required": ["path"],
        },
    },
    {
        "name": "grep",
        "description": "ripgrep over the repo. Returns matching lines with file:line prefix.",
        "input_schema": {
            "type": "object",
            "properties": {
                "pattern": {"type": "string"},
                "glob": {"type": "string", "description": "Optional file glob filter"},
            },
            "required": ["pattern"],
        },
    },
    {
        "name": "glob",
        "description": "List repo files matching a glob pattern.",
        "input_schema": {
            "type": "object",
            "properties": {"pattern": {"type": "string"}},
            "required": ["pattern"],
        },
    },
    {
        "name": "bash",
        "description": "Run a shell command in the repo root. Returns stdout+stderr.",
        "input_schema": {
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"],
        },
    },
]


def arm_env(arm: str) -> dict[str, str]:
    """PATH is the knob that distinguishes arms. Treatment includes sigil's dir."""
    env = os.environ.copy()
    if arm == "control":
        # Strip any directory on PATH that contains a `sigil` binary.
        clean = []
        for d in env.get("PATH", "").split(":"):
            if d and not (Path(d) / "sigil").exists():
                clean.append(d)
        env["PATH"] = ":".join(clean) or "/usr/bin:/bin"
    elif arm == "treatment":
        # Prepend the repo's release build dir if present (so local sigil wins).
        local_bin = REPO_ROOT / "target" / "release"
        if (local_bin / "sigil").exists():
            env["PATH"] = f"{local_bin}:{env.get('PATH', '')}"
    else:
        raise ValueError(f"unknown arm: {arm}")
    return env


def tool_read_file(inp: dict[str, Any], env: dict[str, str], cwd: Path) -> str:
    path = cwd / inp["path"]
    offset = max(1, int(inp.get("offset", 1)))
    limit = max(1, min(int(inp.get("limit", 2000)), 5000))
    if not path.exists():
        return f"ERROR: {path} not found"
    lines = path.read_text(errors="replace").splitlines()
    sliced = lines[offset - 1 : offset - 1 + limit]
    return "\n".join(f"{i + offset:6d}\t{line}" for i, line in enumerate(sliced))


def tool_grep(inp: dict[str, Any], env: dict[str, str], cwd: Path) -> str:
    cmd = ["rg", "--line-number", "--no-heading", inp["pattern"]]
    if inp.get("glob"):
        cmd += ["--glob", inp["glob"]]
    return run_subprocess(cmd, env, cwd)


def tool_glob(inp: dict[str, Any], env: dict[str, str], cwd: Path) -> str:
    # Use a simple `find`-less approach via python glob for reliability.
    from glob import glob
    hits = sorted(glob(inp["pattern"], root_dir=cwd, recursive=True))
    return "\n".join(hits) if hits else "(no matches)"


def tool_bash(inp: dict[str, Any], env: dict[str, str], cwd: Path) -> str:
    return run_subprocess(["bash", "-c", inp["command"]], env, cwd)


def run_subprocess(cmd: list[str], env: dict[str, str], cwd: Path) -> str:
    try:
        proc = subprocess.run(
            cmd, cwd=cwd, env=env, capture_output=True, text=True, timeout=30
        )
    except subprocess.TimeoutExpired:
        return "ERROR: timeout after 30s"
    out = proc.stdout + proc.stderr
    if len(out) > 20000:
        out = out[:20000] + "\n... [truncated]"
    return out or "(no output)"


DISPATCH = {
    "read_file": tool_read_file,
    "grep": tool_grep,
    "glob": tool_glob,
    "bash": tool_bash,
}


def run_one(client, task: dict, arm: str, seed: int, model: str, max_turns: int = 20) -> dict:
    env = arm_env(arm)
    cwd = task_repo_root(task)
    system = SYSTEM_PROMPT_BASE.format(repo=cwd.name)
    if arm == "treatment":
        system += "\n" + SIGIL_BLURB

    messages = [{"role": "user", "content": task["question"]}]
    turns = 0
    tokens_in = 0
    tokens_out = 0
    final_text = None
    trace: list[dict] = []  # compact per-turn record: tool calls + result previews

    while turns < max_turns:
        resp = client.messages.create(
            model=model,
            max_tokens=4096,
            system=system,
            tools=TOOLS,
            messages=messages,
        )
        turns += 1
        tokens_in += resp.usage.input_tokens
        tokens_out += resp.usage.output_tokens

        # Always capture the latest text block, regardless of stop_reason.
        # Lets us grade runs that hit max_tokens mid-reasoning as well as
        # clean end_turn completions — previously those saved as final_text
        # = None and graded as no_answer even when the answer was visible.
        for block in resp.content:
            if block.type == "text":
                final_text = block.text

        if resp.stop_reason == "end_turn":
            break

        if resp.stop_reason == "tool_use":
            assistant_content = [b.model_dump() for b in resp.content]
            messages.append({"role": "assistant", "content": assistant_content})
            tool_results = []
            for block in resp.content:
                if block.type == "tool_use":
                    fn = DISPATCH.get(block.name)
                    if fn is None:
                        result = f"ERROR: unknown tool {block.name}"
                    else:
                        try:
                            result = fn(block.input, env, cwd)
                        except Exception as e:
                            result = f"ERROR: {type(e).__name__}: {e}"
                    # API rejects tool_result content that is an empty string.
                    if not result:
                        result = "(empty)"
                    trace.append({
                        "turn": turns,
                        "tool": block.name,
                        "input": block.input,
                        "result_len": len(result),
                        "result_preview": result[:400],
                    })
                    tool_results.append({
                        "type": "tool_result",
                        "tool_use_id": block.id,
                        "content": result,
                    })
            if not tool_results:
                # Malformed response: stop_reason=tool_use but no tool_use blocks.
                break
            messages.append({"role": "user", "content": tool_results})
            continue

        # Any other stop_reason (max_tokens, refusal) ends the run.
        break

    return {
        "task_id": task["id"],
        "arm": arm,
        "seed": seed,
        "model": model,
        "turns": turns,
        "tokens_in": tokens_in,
        "tokens_out": tokens_out,
        "final_text": final_text,
        "parsed_answer": parse_answer(final_text),
        "trace": trace,
        "timestamp": dt.datetime.now(dt.timezone.utc).isoformat(),
    }


def parse_answer(text: str | None) -> Any | None:
    """Extract a JSON value (array or object) from free-form text.

    Strategy: scan every balanced `[...]` and `{...}` substring, parse
    each, and return the last one that deserializes successfully. The
    "last" bias matches the convention that the final assistant message
    closes with the answer; earlier brackets are usually quoted text
    (e.g. the prompt echoing `#[cfg(test)]` or a JSON snippet in the
    instructions).
    """
    if not text:
        return None
    last_valid: Any = None
    for candidate in _balanced_groups(text):
        try:
            val = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(val, (list, dict)):
            last_valid = val
    return last_valid


def _balanced_groups(text: str) -> list[str]:
    """Yield every balanced `[...]` and `{...}` substring.

    Handles nesting, skips over string literals (so `[` inside a quoted
    string doesn't open a group). Same balancing logic for both bracket
    kinds — useful because tasks may ask for arrays OR objects.
    """
    out: list[str] = []
    stack: list[tuple[str, int]] = []  # (closing_char, start_index)
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            # Skip past the string literal (respect \" escape).
            i += 1
            while i < n and text[i] != '"':
                if text[i] == "\\" and i + 1 < n:
                    i += 2
                    continue
                i += 1
            i += 1
            continue
        if c == "[" or c == "{":
            close = "]" if c == "[" else "}"
            if not stack:
                stack.append((close, i))
            else:
                stack.append((close, -1))  # nested — no new outer start
        elif c in "]}":
            if stack and stack[-1][0] == c:
                close, start = stack.pop()
                if not stack and start != -1:
                    out.append(text[start : i + 1])
            else:
                stack.clear()  # mismatched — reset
        i += 1
    return out


def load_task(path: Path) -> dict:
    with path.open() as f:
        return yaml.safe_load(f)


def _model_slug(model: str) -> str:
    # Short, filesystem-safe tag per model family.
    if "haiku" in model:
        return "haiku-4-5"
    if "sonnet" in model:
        return "sonnet-4-6"
    if "opus" in model:
        return "opus-4-7"
    return model.replace("/", "-").replace(":", "-")


def result_path(date: str, task_id: str, arm: str, seed: int, model: str, task_set: str) -> Path:
    # task_set slug is the short tag (E2, E4, etc.) derived from the
    # task-set dir name by taking the prefix before the first "_".
    slug = task_set.split("_", 1)[0] if "_" in task_set else task_set
    p = RESULTS_ROOT / date / _model_slug(model) / slug / f"{task_id}_{arm}_{seed}.json"
    p.parent.mkdir(parents=True, exist_ok=True)
    return p


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--task", type=Path, help="Single task YAML")
    ap.add_argument("--sweep", action="store_true", help="Run all tasks under --task-set (default: E2_navigation)")
    ap.add_argument("--task-set", default="E2_navigation",
                    help="Task directory under evals/tasks/ (e.g. E2_navigation, E4_swebench_like)")
    ap.add_argument("--arm", choices=["control", "treatment", "both"], default="both")
    ap.add_argument("--seed", type=int, help="Single seed (implies --seeds 1)")
    ap.add_argument("--seeds", type=int, default=3)
    ap.add_argument("--model", default="claude-sonnet-4-6")
    ap.add_argument("--max-turns", type=int, default=20)
    ap.add_argument("--dry-run", action="store_true", help="Print plan, don't call API")
    args = ap.parse_args()

    if args.sweep:
        tasks = sorted((TASKS_ROOT / args.task_set).glob("*.yaml"))
    elif args.task:
        tasks = [args.task]
    else:
        ap.error("must pass --task or --sweep")

    arms = ["control", "treatment"] if args.arm == "both" else [args.arm]
    seeds = [args.seed] if args.seed is not None else list(range(1, args.seeds + 1))
    date = dt.date.today().isoformat()

    plan = [(t, a, s) for t in tasks for a in arms for s in seeds]
    print(f"Plan: {len(plan)} runs  ({len(tasks)} tasks × {len(arms)} arms × {len(seeds)} seeds)  model={args.model}")

    if args.dry_run:
        for t, a, s in plan:
            print(f"  {t.name:50s}  arm={a:9s}  seed={s}")
        return

    from anthropic import Anthropic  # imported lazily so --dry-run works without the SDK
    client = Anthropic()
    random.seed(0)
    random.shuffle(plan)

    for t, a, s in plan:
        task = load_task(t)
        rp = result_path(date, task["id"], a, s, args.model, args.task_set)
        if rp.exists():
            print(f"skip (exists): {rp}")
            continue
        print(f"run: {task['id']}  arm={a}  seed={s}", flush=True)
        try:
            result = run_one(client, task, a, s, args.model, args.max_turns)
        except Exception as e:
            print(f"  ! ERROR: {type(e).__name__}: {e}")
            result = {
                "task_id": task["id"],
                "arm": a,
                "seed": s,
                "model": args.model,
                "turns": 0,
                "tokens_in": 0,
                "tokens_out": 0,
                "final_text": None,
                "parsed_answer": None,
                "error": f"{type(e).__name__}: {e}",
                "timestamp": dt.datetime.now(dt.timezone.utc).isoformat(),
            }
        rp.write_text(json.dumps(result, indent=2))
        print(f"  → tokens_in={result['tokens_in']}  tokens_out={result['tokens_out']}  turns={result['turns']}")


if __name__ == "__main__":
    main()
