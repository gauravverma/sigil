# Notes — sigil on sigil

Worked example: **sigil 0.2.4**, indexed against its own `agent-adoption`
branch at the snapshot where Phase 1 Week 10 just merged. ~2630 entities,
~12900 references, 88 files ranked.

## Wins

1. **PageRank picks the load-bearing file.** Top rank goes to
   `src/entity.rs` (0.0798). That's the file that defines the `Entity`
   and `Reference` structs every other module imports. No tuning; just
   edges from refs.jsonl.

2. **Per-symbol blast surfaces the real dependency story.**
   `blast Entity` reports 48 callers across 16 files, transitive 219
   within depth 3. Reading that before refactoring `Entity` is exactly
   what a reviewer wants. Adding a field to `Entity` touches 11
   construction sites — we found them all via Week-1 sed + rebuild,
   and `blast` would have told us up front.

3. **Duplicates is a cheap win.** The clone report flagged
   `find_sym` (10× across parser language modules) and `extract`
   (8× across same) as duplicated. These are legitimate refactor
   candidates — the tree-sitter extractors share a function shape that
   could hoist into `parser/helpers.rs`. Not planned for this phase
   but genuinely surfaced by the tool.

4. **Context + budget is tight.** `context Entity --budget 1000` fits
   in 358 tokens and gives an agent everything it needs to edit
   `Entity`: signature, callers, callees, related types. The raw
   alternative (read every file that references `Entity`) is 90 KB of
   source, a 252× reduction.

## Misses / rough edges

1. **"Most impactful" section on `sigil review` is crowded with imports.**
   When a commit adds a new module with lots of imports of high-blast
   types, the rank multiplier promotes those imports over the actual
   new code. Example: the `src/context.rs` commit's top-impact list was
   dominated by `std::collections::HashSet`-class lines. Real fix:
   exclude `kind == "import"` from the top-K section. Deferred to
   Phase 1 Week-10.5 polish or Phase 2.

2. **`sigil children src/entity.rs Entity` returns "No symbols found".**
   Sigil's tree-sitter extractor doesn't emit struct fields as child
   entities — the struct is a single entity with a multi-line span.
   For Rust this means `children` is less useful than on OO languages
   where methods live under a class parent. Not a sigil bug; a tree-
   sitter / language convention mismatch.

3. **Test helpers pollute the entity list.** Functions like `ent()`,
   `refr()`, `make_entity()` live in `#[cfg(test)]` modules and show up
   in blast / context / map output. They shouldn't. Phase 2 candidate:
   extend the parser to emit a `test_only: bool` flag and add
   `--exclude-tests` to the query commands.

4. **`sigil context Entity` alternatives list is empty.** Because
   `Entity` is defined exactly once in sigil's source, there's no
   ambiguity — so `alternatives` is an empty section. That's correct,
   but the markdown rendering still prints the `## Ambiguous` heading
   with no body in some edge cases. Minor cosmetic bug.

5. **Co-change misses don't fire on sigil's own recent PRs.** The last
   3 commits were self-contained additions (`sigil blast`, `sigil
   duplicates`, `sigil benchmark` each in their own file + lib.rs +
   main.rs registration). No "expected companion didn't change"
   signal because the companions *did* change. That's the right
   behavior but also means this worked example doesn't demonstrate the
   feature. A more realistic co-change miss would appear on a repo
   with tightly coupled modules (API handler + DTO file, for
   instance).

## Methodology

Token counts in the ratio table are `bytes / 4`, a proxy for modern
tokenizers. Control commands and their measured output sizes:

- **PR review**: `git diff --stat --patch HEAD~3..HEAD` → 185,954 tokens
- **Context `Entity`**: read all 17 files that reference `Entity`
  (bounded at 100 in the benchmarker) → 90,296 tokens
- **Cold-start**: cat the first 20 entities-having files → 47,879 tokens

Sigil output sizes measured from the actual stdout of the equivalent
command. Nothing staged; `evals/run.sh` is reproducible.

## Reproducibility

```bash
git clone https://github.com/gauravverma/sigil
cd sigil
git checkout agent-adoption
cargo run --release -- index
cargo run --release -- map --tokens 3000 --write
cargo run --release -- review HEAD~3..HEAD > worked/sigil-self/review-HEAD~3..HEAD.md
# ...etc
```

Numbers match the `evals/results/0.2.4-HEAD-3..HEAD.json` snapshot
within ±5%.
