# sigil evals

The minimum honest benchmark: for a fixed set of "agent-shaped" queries, how
many tokens does it take to answer via **raw tools** (git, grep, file reads)
vs **sigil commands**?

This directory holds the scripts, inputs, and historical results. The
methodology is deliberately simple at this stage — no LLM in the loop, no
hand-labeled ground truth, no SWE-bench. Just a reproducible
token-accounting pass on a real repo (sigil itself) whose numbers we can
publish without hand-waving.

Full eval design lives in **§8 of `agent-adoption-plan.md`**. This
directory is the Week-11 / Week-12 slice: the smoke-level benchmark that
shipped first so later work (E1 PR review with LLM-judge, E2 navigation
precision/recall, E4 SWE-bench) has somewhere to land.

## Layout

```
evals/
  README.md              ← this file: methodology + how to run
  run.sh                 ← capture a new result snapshot
  results/               ← one JSON per (sigil_version, refspec)
```

## What's measured today

`sigil benchmark --format json` runs three canonical queries:

| Query | Control (raw) | Treatment (sigil) |
|---|---|---|
| **PR review** | `git diff --stat --patch <refspec>` | `sigil review <refspec>` |
| **Context for `<sym>`** | read every file that references `<sym>` (bounded 100) | `sigil context <sym>` |
| **Cold-start orientation** | cat 20 random source files | `sigil map --tokens 2000` |

For each, we capture the output of both approaches, estimate tokens as
`bytes / 4` (a stable proxy for modern tokenizers, ±20% in either
direction), and publish the ratio. The median across queries is the
headline number.

## What's *not* measured yet

- **Agent success rate** — no model in the loop. A win on tokens means
  little if the agent can't reach an answer. Covered by E1/E2/E4 in §8
  of the plan.
- **Cross-repo variance** — we only run on sigil itself. Different
  codebases will produce different numbers; especially, token reduction
  grows with corpus size (graphify's 71.5× on Karpathy-repos vs ~5×
  on a single lib). Covered by E1 batching in §8.
- **Hand-labeled navigation ground truth** — comparing sigil's caller
  set vs grep's would need a human oracle per symbol. Deferred.

## Why `bytes / 4`

Picked deliberately: real tokenizers (tiktoken, Claude's) vary by model
and language, but all cluster around 3–5 bytes per token on code.
bytes/4 is stable across Rust/Python/TS/Go and doesn't require a model
dependency. When we start reporting dollar figures, the eval harness
swaps in a real tokenizer (behind a feature flag — see §14.9 of the
plan for the scale story).

## How to capture a snapshot

```bash
# Make sure the index is fresh and rank is populated
cargo run --release -- index

# Capture with the current HEAD range
./evals/run.sh

# Or with an explicit refspec
./evals/run.sh HEAD~5..HEAD
```

The script writes `evals/results/<sigil-version>-<refspec>.json` and
prints the median ratio to stderr. Commit the JSON alongside the code
change that produced it — each result is a dated artifact.

## Cross-repo benchmark

The sigil-self number is one data point. Real adoption decisions need
multiple points across corpus sizes, languages, and coupling densities.
`evals/cross_repo.sh` runs the benchmark across a curated OSS set.

```bash
# Defaults: uses evals/corpus.tsv, persists results to
# evals/results/cross-repo-<date>/.
./evals/cross_repo.sh

# Custom corpus:
./evals/cross_repo.sh path/to/my-corpus.tsv

# Persist clones across runs (skips re-cloning on repeat):
CORPUS_DIR=/tmp/sigil-corpus ./evals/cross_repo.sh
```

The script clones each repo (shallow, depth 200), runs `sigil index`,
and records a benchmark JSON per repo. At the end it emits a
`README.md` summary table with per-repo medians and per-query detail.

**Expected runtime**: ~1 minute per repo on cold clone. Under 5 minutes
for the default 3-repo corpus on a decent connection.

### Adding repos

Append a tab-separated row to `evals/corpus.tsv`:

```
<slug>	<git-url>	<ref>	<refspec>
```

Keep refs pinned (tag or SHA, not `main`) so results are reproducible
on re-run. Test corpora belong in their own TSV, not the main one.

## Historical results

| Date | sigil version | refspec | Median ratio | PR review | Context | Orientation |
|---|---|---|---:|---:|---:|---:|
| 2026-04-20 | 0.2.4 | HEAD~3..HEAD | 25.91× | 25.91× | 252.22× | 25.32× |

Raw JSON: [`results/0.2.4-HEAD-3..HEAD.json`](results/0.2.4-HEAD-3..HEAD.json)

## Reproducibility

```bash
git clone https://github.com/gauravverma/sigil
cd sigil
cargo run --release -- index
./evals/run.sh HEAD~3..HEAD
# Compare against evals/results/0.2.4-HEAD-3..HEAD.json
```

Numbers should match within ±5% of the published result on the same git
ref. Larger deviations typically mean the repo has evolved; re-run
against a specific commit SHA to reproduce historical numbers.
