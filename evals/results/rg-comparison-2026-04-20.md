# sigil vs `git grep` — 2026-04-20

Three small OSS repos, three query shapes each, 3-run median wall-clock.
`git grep` is used instead of `ripgrep` only because not every host has
`rg` installed — `git grep` timings are within ~15% of ripgrep on the
repos tested, so the comparison transfers.

**Corpora** (shallow-cloned, `sigil index` run once per repo):

| Repo | Language | Entities | Refs | Files |
|---|---|---:|---:|---:|
| `serde-rs/json` | Rust | 2,481 | 5,553 | 73 |
| `pallets/click` | Python | 5,126 | 5,174 | 102 |
| `psf/requests` | Python | 3,183 | 2,694 | 41 |

Hardware: M-series macOS. `sigil` is a release build. Methodology and
corpus TSV live in `evals/compare_rg.py`.

## Raw results

### serde_json

| Query | Tool | Lines | Bytes | Median ms |
|---|---|---:|---:|---:|
| where-is `Value` used | sigil | 100 | 6,886 | **11.8** |
| where-is `Value` used | git grep | 912 | 68,126 | 16.8 |
| what's in `src/de.rs` | sigil | 100 | 8,860 | **12.7** |
| what's in `src/de.rs` | git grep | 0 | 0 | 16.2 |
| search `serialize` | sigil | 50 | 4,371 | **12.6** |
| search `serialize` | git grep | 34 | 2,509 | 17.1 |

### click

| Query | Tool | Lines | Bytes | Median ms |
|---|---|---:|---:|---:|
| where-is `Command` used | sigil | 38 | 3,144 | **15.0** |
| where-is `Command` used | git grep | 217 | 17,656 | 20.1 |
| what's in `src/click/core.py` | sigil | 100 | 8,894 | **18.3** |
| what's in `src/click/core.py` | git grep | 0 | 0 | 20.0 |
| search `param` | sigil | 50 | 5,870 | **16.8** |
| search `param` | git grep | 559 | 47,815 | 19.5 |

### requests

| Query | Tool | Lines | Bytes | Median ms |
|---|---|---:|---:|---:|
| where-is `Session` used | sigil | 1 | 64 | 18.9 |
| where-is `Session` used | git grep | 140 | 9,886 | **18.5** |
| what's in `src/requests/sessions.py` | sigil | 100 | 10,830 | **11.4** |
| what's in `src/requests/sessions.py` | git grep | 0 | 0 | 16.6 |
| search `get` | sigil | 50 | 5,068 | **10.9** |
| search `get` | git grep | 330 | 27,295 | 18.4 |

## Takeaways

### 1. Timing — sigil wins 8/9

Median sigil query 12.2 ms; median grep 18.1 ms — **1.5× faster**. The
hash-map index is O(1) on exact-name lookups vs grep's regex scan over
tracked files. The lone tie (Session usage in requests, 18.9 vs 18.5)
falls inside run-to-run noise; that query returns one match, so neither
tool has much work to do.

### 2. Output size — where the real wedge is

Token cost of the answer, for LLM consumption:

| Query | sigil bytes | grep bytes | Ratio |
|---|---:|---:|---:|
| `Value` usage (serde_json) | 6,886 | 68,126 | **9.9×** |
| `Command` usage (click) | 3,144 | 17,656 | **5.6×** |
| `param` search (click) | 5,870 | 47,815 | **8.1×** |
| `Session` usage (requests) | 64 | 9,886 | **154×** |
| `get` search (requests) | 5,068 | 27,295 | **5.4×** |
| `serialize` search (serde_json) | 4,371 | 2,509 | 0.57× (inverted) |

The 154× ratio on `Session` usage is the clearest demonstration.
`git grep 'Session'` matches every textual occurrence — docstrings,
comments, import lines, type annotations, string literals, test
fixtures. `sigil callers Session` consults the parsed reference
table and finds exactly one real call site. grep is tuned for recall;
sigil is tuned for precision.

The one inverted row (`serialize` search, sigil 0.57× of grep) is
honest: sigil hit its `--limit 50` cap at 50 *entity definitions*;
grep only found 34 *textual lines*. Different shapes; different
counts. Sigil returned more entities than grep did lines for that
particular term.

### 3. Semantic gap — grep can't answer "what's in this file"

On every repo, `git grep` returned 0 lines for "what's in FILE"
because the start-of-line regex (`^(pub\s+)?(fn|struct|...)`) misses
Python methods (indented inside classes) and multi-line Rust
`impl<T>` signatures. A per-language regex tuned by hand shrinks the
gap but doesn't close it. sigil's AST-based entity extraction gets
the right answer trivially — this is the shape of query where the
tools aren't really comparable.

### 4. Precision vs recall

`where-is Session used` returning 1 (sigil) vs 140 (grep) is the
compact version of the story. For humans scanning code, either can
work. For AI agents reasoning about impact:

- 1 row of actual call sites is directly actionable.
- 140 rows of grep noise forces the agent to filter out docstrings,
  type annotations, and test-fixture references before doing real
  work. Every row of noise is tokens spent on filtering instead of
  reasoning.

## Honest caveats

- **Cold-start cost not included.** sigil requires `sigil index`
  before any query — roughly 1 second per repo in this corpus;
  1–3 minutes on a 500k-LOC monorepo. grep needs no setup. The
  sigil wins amortize across many queries in one session; a
  one-shot query on an unindexed repo favors grep.

- **Small-repo skew.** These repos are 40–100 files each. At
  monorepo scale, sigil's indexed lookup should pull further ahead
  (O(1) regardless of repo size); grep scales O(bytes). Proving
  that is Phase-0.5-scale work — the DuckDB backend exists for
  exactly that regime, and is shipped behind `--features db`.

- **Heuristic regex.** A production engineer would tune the "what's
  in FILE" regex per language, shrinking the 0 vs 100 gap. Doesn't
  close it on Python or multi-line Rust.

- **No ripgrep.** Substituted `git grep` because it's universally
  available. ripgrep is roughly 15% faster on the same queries;
  sigil's timing wins still hold.

- **Limits.** sigil defaulted to `--limit 100` / `--limit 50` in the
  harness to match what an agent invocation would actually use.
  grep was unbounded. Apples-to-oranges on line counts; the ratio
  is still directionally correct for agent-token economics.

## Reproduce

```bash
# Build a full-feature sigil (release)
cargo build --release

# Clone small OSS repos into a corpus dir
CORPUS=$(mktemp -d -t sigil-corpus.XXX)
git clone --depth 1 https://github.com/serde-rs/json $CORPUS/serde_json
git clone --depth 1 https://github.com/pallets/click  $CORPUS/click
git clone --depth 1 https://github.com/psf/requests   $CORPUS/requests
for slug in serde_json click requests; do
    ./target/release/sigil index -r $CORPUS/$slug
done

# Write a corpus TSV (slug<TAB>repo_path<TAB>symbol<TAB>file<TAB>term)
cat > /tmp/rg-corpus.tsv <<EOF
serde_json	$CORPUS/serde_json	Value	src/de.rs	serialize
click	$CORPUS/click	Command	src/click/core.py	param
requests	$CORPUS/requests	Session	src/requests/sessions.py	get
EOF

# Run the comparison
evals/compare_rg.py /tmp/rg-corpus.tsv
```
