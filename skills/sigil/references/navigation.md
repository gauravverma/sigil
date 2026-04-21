# Navigation primitives — reference

The script-facing layer. Use these when the one-shot agent-facing commands (`sigil where`, `sigil context`, `sigil callers`, etc.) in SKILL.md don't fit — e.g., scripted "list every caller of X" rather than "what does X do."

All script-facing commands default to **unbounded results** (`--limit 0`) and **minified JSON** with `--json`. Add `--pretty` only for human inspection.

## `sigil search` — substring match over names

```bash
sigil search "handler" --json                # symbol hits by name substring
sigil search "MyClass" --scope file          # filename matches
sigil search "build" --kind function         # kind filter
sigil search "config" --path "src/*.rs"      # path filter
sigil search "parse" --scope all --limit 50  # broader + cap
```

- `--scope` defaults to `symbol`. Use `file` or `all` to widen.
- `--kind` filters to one of `function | class | method | struct | enum | trait` (and a few more).
- Each row carries `file`, `line`, `kind`, `parent`, and a `sig` preview when the entity has one — saves a follow-up `read_file` for signature lookups.
- Overloads (same `(file, name, kind)` appearing multiple times, e.g. Python `@overload` stubs) collapse into one row with `overloads: N`.

Empty results print `Did you mean: X, Y, Z?` on stderr when the queried name is close to a known entity. Retry with a suggestion before falling back to grep.

## `sigil symbols` — entities in a file

```bash
sigil symbols src/main.rs --json                          # all entities in the file
sigil symbols src/main.rs --depth 1 --json                # top-level only (drop nested)
sigil symbols src/main.rs --depth 1 --names-only          # flat array of tail names — ~90% smaller
sigil symbols "src/*.rs" --json                           # glob patterns
sigil symbols src/main.rs --with-hashes --json            # include BLAKE3 cols (rare need)
```

- `--depth 1` keeps only top-level items (classes, top-level fns, structs, enums, traits, sections) — drops imports, variables, constants, nested methods. ~95% size drop on mid-sized files.
- `--names-only` emits just `["Foo","Bar","Baz"]` — the right shape when the answer is a list of names, not entity records. ~3 KB → ~300 bytes typical.

## `sigil children` — entities under a parent

```bash
sigil children src/entity.rs Entity --json   # methods/fields under class Entity
```

Rarely needed — `sigil symbols` usually suffices.

## Call graph — `sigil callers` / `sigil callees`

```bash
sigil callers process --json                            # all references targeting `process`
sigil callers process --kind call --json                # call-sites only
sigil callers build_index --kind import --json          # only import references
sigil callers foo --group-by file                       # {file: count} aggregation
sigil callers foo --group-by caller                     # {caller_fn: count}

sigil callees build_index --json                        # what does build_index call?
sigil callees build_index --group-by name               # count per target
```

- Valid `--kind` values: `call` | `import` | `type_annotation` | `instantiation`.
- `--group-by` collapses per-call-site output to a `{key: count}` map. Valid keys: `file` | `caller` | `name` | `kind`. Useful when you only need distribution, not line-level detail — turns a 128-ref flat list into a few-entry summary.
- **Qualified-tail match**: `sigil callers parse_file` also finds refs stored as `crate::parser::treesitter::parse_file`. You rarely need to pass the qualified form; the bare tail segment is enough.

## `sigil explore` — directory overview

```bash
sigil explore --json                      # whole project structure
sigil explore --path src/parser --json    # subtree
```

Returns directory listings with file counts by language. For "what's in this directory" **structurally** (classes + functions, not raw files), use `sigil outline --path DIR` from SKILL.md instead.

## When to reach for navigation primitives vs the one-shot commands

| You want | Use | Not |
|---|---|---|
| "where is `X` defined?" | `sigil where X` (in SKILL.md) | `sigil search X --scope symbol` |
| "full bundle for `X`" | `sigil context X` | chained `search + callers + callees` |
| "who calls `X`?" | `sigil callers X` (in SKILL.md) | grep for `X\s*(` |
| "list names of structs in file F" | `sigil symbols F --depth 1 --names-only` | full dump then jq |
| "list files in a directory" | `ls` / `find` / `bash` | `sigil outline` (that's for classes+fns) |

The navigation primitives are here for cases where the one-shot commands don't fit — scripted extraction, narrow filters, or composable pipelines.
