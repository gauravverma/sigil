# Advanced: duplicates, SQL, benchmarks, co-change

Rarely-needed commands. Reach for these when the questions in the main cheat-sheet don't fit.

## `sigil duplicates` — clone detection

Groups entities by `body_hash` — literal duplicate implementations anywhere in the repo.

```bash
sigil duplicates                            # markdown report
sigil duplicates --min-lines 10             # ignore small fragments
sigil duplicates --format json              # structured
sigil duplicates --max-group-size 20        # drop huge groups (usually generated code)
```

Useful when the user asks:
- "where's this copy-pasted?"
- "is this already implemented somewhere?"
- during cleanup / consolidation refactors
- as a clone-introduction signal inside `sigil review` (runs automatically there)

## `sigil query "SQL"` — DuckDB escape hatch

Ad-hoc SQL against the materialized index. Shipped binaries include DuckDB — no extra setup or feature flags.

```bash
# Row count by kind
sigil query "SELECT kind, COUNT(*) FROM entities GROUP BY 1 ORDER BY 2 DESC"

# Public, high-rank entities
sigil query "SELECT name, file FROM entities WHERE visibility = 'public' AND rank > 0.01"

# Every reference to a specific target
sigil query "SELECT * FROM refs WHERE name = 'parse_file' LIMIT 50" --format json

# Public functions with no callers
sigil query "SELECT e.name, e.file FROM entities e
             LEFT JOIN refs r ON r.name = e.name
             WHERE e.kind = 'function' AND e.visibility = 'public' AND r.name IS NULL"

# Files with the most entities
sigil query "SELECT file, COUNT(*) AS n FROM entities GROUP BY file ORDER BY n DESC LIMIT 20"
```

Schema:

| Table | Columns |
|---|---|
| `entities` | `file, name, kind, line_start, line_end, parent, sig, body_hash, sig_hash, struct_hash, visibility, rank, blast_radius` |
| `refs` | `file, caller, name, kind, line` |

Views: `rank` (file-level PageRank scores), `blast` (per-entity blast radius fields, denormalized).

Flags:
- `--format markdown` (default) or `--format json`
- `--pretty` for indented JSON
- `--max-cell-width N` to truncate wide cells in markdown output

Use this when the question doesn't fit the built-in commands — cross-joins, aggregations, top-K queries.

## `sigil benchmark` — token accounting

Publishes a median token-reduction number for your repo, comparing sigil output bytes vs raw alternatives (git log, git diff, ls + file reads).

```bash
sigil benchmark                                           # bytes/4 proxy (fast)
sigil benchmark --tokenizer o200k_base                    # BPE-accurate (GPT-4o)
sigil benchmark --tokenizer cl100k_base                   # BPE (GPT-3.5/4 legacy)
sigil benchmark --refspec HEAD~5..HEAD --symbol Entity    # specific inputs
sigil benchmark --format json                             # structured
```

Useful only when the user explicitly asks about token efficiency. The published numbers assume BPE (`o200k_base`); the default `proxy` tokenizer over-estimates ratios by 15-30%.

## `sigil cochange` — rebuild co-change cache

Mines git history for file-pair co-change weights. Writes `.sigil/cochange.json`. `sigil review` consumes this automatically — rarely need to run directly unless the cache is stale or the repo has new history since last build.

```bash
sigil cochange                              # default 500 commits
sigil cochange --commits 2000               # wider history
```

## `sigil index` — rebuild the structural index

Most agents don't need to invoke this directly — sigil auto-runs it on the first query in a repo without `.sigil/` (see the "Zero-config onboarding" section in SKILL.md).

```bash
sigil index                                 # incremental rebuild (default)
sigil index -v                              # progress to stderr
sigil index --full                          # force full reparse (ignore cache)
sigil index --no-rank                       # skip PageRank + blast-radius pass
```

The `--no-rank` flag is useful in CI where you want the index artifacts fast and don't need rank-based ranking commands. Subsequent `sigil map` / `sigil blast` calls will be degraded without rank data — re-run without `--no-rank` before using them.

## Install / hook commands (for operators, not agents)

```bash
sigil claude install     sigil claude uninstall       # CLAUDE.md block + hooks
sigil cursor install     sigil cursor uninstall       # .cursor/rules/sigil.mdc
sigil codex install      sigil codex uninstall        # AGENTS.md + .codex/hooks.json
sigil gemini install     sigil gemini uninstall       # GEMINI.md + .gemini/settings.json
sigil opencode install   sigil opencode uninstall     # AGENTS.md + .opencode/plugins/
sigil aider install      sigil aider uninstall        # AGENTS.md block
sigil copilot install    sigil copilot uninstall      # ~/.copilot/skills/sigil/SKILL.md
sigil hook install       sigil hook uninstall         # git post-commit auto-index hook
```

All installers are idempotent (re-run upgrades the block in place using `<!-- sigil:begin --> … <!-- sigil:end -->` sentinel markers) and preserve user content outside the markers.
