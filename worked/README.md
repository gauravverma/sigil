# Worked examples

Real `sigil` output on real repositories, alongside honest notes on what it
got right and what it missed. The intended audience is someone evaluating
sigil for their own workflow who wants to see end-to-end artifacts before
installing.

Trust-building rule: every example must include a notes section calling
out misses, not just wins. Cherry-picking erodes the one thing sigil has
that LLM-graph tools don't: determinism.

## Layout

```
worked/
  README.md               ← this file
  sigil-self/             ← sigil indexed against its own source
    SIGIL_MAP.md          ← `sigil map --tokens 3000 --write`
    review-HEAD~3..HEAD.md ← `sigil review HEAD~3..HEAD`
    context-Entity.md     ← `sigil context Entity --budget 1000`
    blast-Entity.md       ← `sigil blast Entity --depth 5`
    duplicates.md         ← `sigil duplicates --min-lines 5`
    notes.md              ← honest evaluation: wins and misses
```

## sigil-self

The first worked example: sigil's own source (~2600 entities across
88 files). Generated from the `agent-adoption` branch at the v0.2.4
tag range.

Benchmark numbers captured in `evals/results/0.2.4-HEAD-3..HEAD-o200k.json`
(BPE-accurate counts via `sigil benchmark --tokenizer o200k_base`):

| Query | Raw tokens | Sigil tokens | Ratio |
|---|---:|---:|---:|
| PR review (3 commits) | 195,003 | 5,572 | 35.00× |
| Context for `Entity` | 91,937 | 467 | 196.87× |
| Cold-start orientation | 44,733 | 2,786 | 16.06× |

Median reduction: **35.00×** (`o200k_base` tokenizer, GPT-4o/o3).

Read the artifacts in order:

1. [`SIGIL_MAP.md`](sigil-self/SIGIL_MAP.md) — cold-start orientation.
2. [`context-Entity.md`](sigil-self/context-Entity.md) — focused bundle
   for the repo's most-referenced type.
3. [`blast-Entity.md`](sigil-self/blast-Entity.md) — impact summary
   (48 callers, 16 files, transitive 219).
4. [`review-HEAD~3..HEAD.md`](sigil-self/review-HEAD~3..HEAD.md) — PR
   review artifact for the last three commits.
5. [`duplicates.md`](sigil-self/duplicates.md) — clone report; surfaces
   `find_sym` and `extract` duplicated across parser-language modules
   (legitimate refactor target).
6. [`notes.md`](sigil-self/notes.md) — wins / misses / rough edges.

## Contributing more examples

A good worked example:

- Uses a real public repo at a specific commit SHA (reproducible).
- Ships every artifact `sigil` generates (map, review, context on a
  representative symbol, duplicates at default thresholds).
- Includes a `notes.md` with at least one miss or rough edge.
- Fits under 50 KB total — these are meant to be scannable, not
  exhaustive dumps.

Open a PR adding `worked/<repo-slug>/` with the files above. The one
mandatory thing is honesty in `notes.md` — cherry-picked "clean win"
contributions will be closed.
