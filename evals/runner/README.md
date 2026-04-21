# Eval runner v0.0.1 — E2 navigation only

Minimal harness for the question the tokens-only benchmark can't answer:
**does the agent actually reach the right answer, and how many tokens does it
cost?**

## Scope (deliberately small)

- **1 task set**: `E2_navigation` (callers / callees / symbols in a file).
  Answers are sets of strings → deterministic grading, no LLM judge.
- **2 arms**: `control` (Read + Grep + Glob + Bash without sigil on PATH)
  and `treatment` (same tools, sigil on PATH, short capability blurb in
  the system prompt).
- **1 model**: Sonnet 4.6 for the reported number. Iterate on Haiku 4.5
  to debug the loop cheaply.
- **N seeds**: 3 by default. Raise only when CI is borderline.

Expected cost for a 10-task × 2-arm × 3-seed × Sonnet run: **~$5–10**.
Haiku iteration: ~$1–2.

## Layout

```
evals/
  runner/
    run.py            agent loop (Anthropic SDK tool_use)
    grade.py          set-match grading
    requirements.txt
  tasks/
    E2_navigation/
      001-*.yaml      {id, pinned_ref, question, expected, grader}
  results/
    <date>/E2/<task>_<arm>_<seed>.json
    <date>/E2/summary.md
```

## Running

```bash
# One-time
pip install -r evals/runner/requirements.txt
export ANTHROPIC_API_KEY=...

# Single task, single arm, single seed (smoke test on Haiku)
python evals/runner/run.py \
  --task evals/tasks/E2_navigation/001-callers-of-parse-file.yaml \
  --arm treatment --seed 1 --model claude-haiku-4-5

# Full sweep on Sonnet (the published number)
python evals/runner/run.py --sweep --model claude-sonnet-4-6 --seeds 3

# Grade + summarize the latest run
python evals/runner/grade.py results/<date>/E2
```

Each task is pinned to a specific git ref so re-runs are reproducible
across repo evolution. Keep refs as tags or SHAs, never `main`.

## Fairness rules (inherited from §8.6 of the plan)

1. Task prompts never mention sigil by name. Treatment arm gets it via
   `PATH` + a tool-description blurb, matching production use.
2. Random task order per seed (avoid ordering artifacts).
3. Model version pinned per run and recorded in result JSON.
4. Task corpus frozen per release.

## What this doesn't cover yet

Everything else in §8 of the plan:

- E1 (PR review) — needs an LLM judge for the qualitative parts
- E3 (orientation), E4 (SWE-bench Lite), E5 (targeted edit)
- Ablation arms (map-only, context-only, diff-only, callers-only)
- Opus headline pass

Scope those in when the E2 pipeline is green.
