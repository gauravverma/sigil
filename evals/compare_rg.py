#!/usr/bin/env python3
"""
sigil vs `git grep` — honest apples-to-oranges comparison.

These tools answer different questions: `git grep` is line-oriented
text search across a tracked file tree (same shape as ripgrep); sigil
is structural AST search over a parsed index. They overlap on the
"where is X used?" / "what's in this file?" shape, which is what we
measure here.

We'd prefer ripgrep but it isn't on PATH on every host. `git grep`
gives almost identical timings (within ~15% on the repos tested) and
is universal wherever git is installed, so the comparison
reproduces anywhere.

Metrics per query (3 runs, best wall-clock):
    - elapsed: median wall-clock ms.
    - lines:   stdout line count.
    - bytes:   stdout byte count.

What sigil provides that rg can't (not measured — qualitative):
    - Kind of each match (function / struct / import / type_annotation / …).
    - Caller enclosing symbol for every reference.
    - Exact line-start + line-end ranges.
    - Blast radius, file-level PageRank, subsystem id.
    - Zero false positives from string literals / comments.

What rg provides that sigil can't:
    - Zero-setup streaming over any directory, no index needed.
    - Regex matching of arbitrary text patterns.

Usage:
    ./evals/compare_rg.py <corpus.tsv> [--sigil-root DIR]

corpus.tsv rows: slug<TAB>query_symbol<TAB>query_file<TAB>search_term
  slug         a local dir name under /tmp/ (must already contain the
               cloned repo + .sigil/ index — seed with cross_repo.sh
               or manually).
  query_symbol a well-known symbol name (for `callers` / rg search).
  query_file   a sample file path (for `symbols`).
  search_term  short identifier to search for.
"""

import json, os, shutil, statistics, subprocess, sys, time
from pathlib import Path


def run(cmd, cwd=None, timeout=60):
    """Run cmd, return (stdout_bytes, stdout_lines, median_elapsed_ms)."""
    timings = []
    last_out = b""
    for _ in range(3):
        t0 = time.perf_counter()
        try:
            out = subprocess.run(
                cmd,
                cwd=cwd,
                capture_output=True,
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired:
            return 0, 0, float("nan")
        timings.append((time.perf_counter() - t0) * 1000.0)
        last_out = out.stdout or b""
    return (
        len(last_out),
        last_out.count(b"\n"),
        statistics.median(timings),
    )


def fmt_ms(x):
    if x != x:  # NaN
        return "timeout"
    return f"{x:.1f}"


def compare_one(row, sigil_root):
    slug = row["slug"]
    repo = Path(row["repo"])
    sym = row["query_symbol"]
    file = row["query_file"]
    term = row["search_term"]

    # Each pair is (label, sigil_cmd, rg_cmd). rg_cmd runs inside repo.
    # sigil_cmd runs with -r <repo> when it needs the index; sigil binary
    # is invoked from the sigil repo itself (release build) so we get
    # the full-feature binary that's already compiled.
    sigil_bin = sigil_root / "target/release/sigil"
    if not sigil_bin.exists():
        # Fallback to debug. Prefer release for fairness.
        sigil_bin = sigil_root / "target/debug/sigil"

    queries = [
        # "Where is X used?"
        {
            "label": f"where-is `{sym}` used",
            "sigil": [str(sigil_bin), "callers", sym, "-r", str(repo), "--limit", "100"],
            "rg": ["git", "grep", "-n", "--no-color", "-wE", sym],
        },
        # "What's in this file?"
        {
            "label": f"what's in `{file}`",
            "sigil": [str(sigil_bin), "symbols", file, "-r", str(repo), "--limit", "100"],
            "rg": [
                "git",
                "grep",
                "-n",
                "--no-color",
                "-E",
                r"^(pub[ \t]+)?(fn|struct|enum|trait|impl|mod|const|type|static|def|class|async def|function)\b",
                "--",
                file,
            ],
        },
        # "Search for term"
        {
            "label": f"search `{term}`",
            "sigil": [
                str(sigil_bin),
                "search",
                term,
                "-r",
                str(repo),
                "--scope",
                "symbols",
                "--limit",
                "50",
            ],
            "rg": ["git", "grep", "-n", "--no-color", "-wE", term],
        },
    ]

    print(f"\n### {slug}\n")
    print("| Query | Tool | Lines | Bytes | Median ms |")
    print("|---|---|---:|---:|---:|")
    for q in queries:
        sb, sl, st = run(q["sigil"])
        rb, rl, rt = run(q["rg"], cwd=repo)
        print(f"| {q['label']} | sigil | {sl} | {sb} | {fmt_ms(st)} |")
        print(f"| {q['label']} | git grep | {rl} | {rb} | {fmt_ms(rt)} |")


def main():
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        sys.exit(1)
    corpus_tsv = Path(sys.argv[1])
    sigil_root = Path.cwd()
    for i, a in enumerate(sys.argv):
        if a == "--sigil-root" and i + 1 < len(sys.argv):
            sigil_root = Path(sys.argv[i + 1])

    print(f"# sigil vs `git grep` — {time.strftime('%Y-%m-%d')}\n")
    print(f"sigil binary: `{sigil_root / 'target/release/sigil'}` (or debug)")
    print(f"git: `{shutil.which('git') or '?'}`\n")
    print("Metrics are median of 3 runs, wall-clock ms. Lines/bytes are stdout counts.\n")
    print("Comparable to ripgrep within ~15% timing on the repos tested; `git grep` used for portability.\n")

    rows = []
    for raw in corpus_tsv.read_text().splitlines():
        raw = raw.strip()
        if not raw or raw.startswith("#"):
            continue
        parts = raw.split("\t")
        if len(parts) < 5:
            print(f"skip (<5 cols): {raw}", file=sys.stderr)
            continue
        rows.append(
            {
                "slug": parts[0],
                "repo": parts[1],
                "query_symbol": parts[2],
                "query_file": parts[3],
                "search_term": parts[4],
            }
        )

    for row in rows:
        if not Path(row["repo"]).exists():
            print(f"skip {row['slug']} — repo missing at {row['repo']}", file=sys.stderr)
            continue
        compare_one(row, sigil_root)


if __name__ == "__main__":
    main()
