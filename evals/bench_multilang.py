#!/usr/bin/env python3
"""
Multi-language end-to-end benchmark: from `sigil index` cold-start
through the three query shapes we care about (`callers`, `symbols`,
`search`), compared to `git grep` equivalents across Rust / Python /
TypeScript / Go.

Usage:
    ./evals/bench_multilang.py <corpus.tsv>

corpus.tsv rows (tab-separated):
    slug   repo_path   language   symbol   file_for_symbols   search_term

This script:
    1. Times `sigil index --no-rank` on each repo (cold-start cost).
    2. Runs 3 query shapes with both sigil and `git grep`, 3 runs
       each, reports median ms + lines + bytes.
    3. Emits markdown tables suitable for `evals/results/`.
"""

import shutil, statistics, subprocess, sys, time
from pathlib import Path


def run(cmd, cwd=None, timeout=120, runs=3):
    """Run cmd `runs` times, return (bytes, lines, median_ms, failed_bool)."""
    timings = []
    last_out = b""
    failed = False
    for _ in range(runs):
        t0 = time.perf_counter()
        try:
            r = subprocess.run(cmd, cwd=cwd, capture_output=True, timeout=timeout)
            if r.returncode != 0:
                failed = True
        except subprocess.TimeoutExpired:
            return 0, 0, float("nan"), True
        timings.append((time.perf_counter() - t0) * 1000.0)
        last_out = r.stdout or b""
    return (
        len(last_out),
        last_out.count(b"\n"),
        statistics.median(timings),
        failed,
    )


def fmt_ms(x):
    if x != x:
        return "timeout"
    return f"{x:.1f}"


def fmt_ratio(s, g):
    if g == 0:
        return "n/a"
    return f"{g / s:.2f}×" if s > 0 else "n/a"


def run_bench(row, sigil_bin):
    slug, repo, lang, sym, file, term = (
        row["slug"],
        Path(row["repo"]),
        row["language"],
        row["symbol"],
        row["file"],
        row["term"],
    )
    # Ensure a clean .sigil/ so we measure cold init.
    sigil_dir = repo / ".sigil"
    if sigil_dir.exists():
        shutil.rmtree(sigil_dir)

    # 1. Init (`sigil index`) — cold-start cost.
    init_cmd = [str(sigil_bin), "index", "--no-rank", "-r", str(repo), "--verbose"]
    init_out, _, init_ms, init_failed = run(init_cmd, runs=1, timeout=180)
    # Extract entity/ref counts from the verbose stderr on success; on
    # failure we print what we have.
    entities = refs = files = 0
    try:
        # Read the fresh index for canonical counts (avoids parsing
        # verbose output).
        import json

        with open(repo / ".sigil/entities.jsonl") as f:
            entities = sum(1 for _ in f)
        if (repo / ".sigil/refs.jsonl").exists():
            with open(repo / ".sigil/refs.jsonl") as f:
                refs = sum(1 for _ in f)
        files = len({json.loads(l)["file"] for l in open(repo / ".sigil/entities.jsonl")})
    except Exception:
        pass

    # 2. Query comparisons (3 shapes).
    queries = [
        ("callers", f"where-is `{sym}` used", sym),
        ("symbols", f"what's in `{file}`", file),
        ("search", f"search `{term}`", term),
    ]

    # Sigil commands.
    sigil_cmds = {
        "callers": [str(sigil_bin), "callers", sym, "-r", str(repo), "--limit", "100"],
        "symbols": [str(sigil_bin), "symbols", file, "-r", str(repo), "--limit", "100"],
        "search": [
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
    }
    # git grep equivalents.
    grep_cmds = {
        "callers": ["git", "grep", "-n", "--no-color", "-wE", sym],
        "symbols": [
            "git",
            "grep",
            "-n",
            "--no-color",
            "-E",
            r"^(pub[ \t]+)?(fn|struct|enum|trait|impl|mod|const|type|static|def|class|async[ \t]+def|function|export[ \t]+(class|function|const)|type[ \t]+\w+|interface)\b",
            "--",
            file,
        ],
        "search": ["git", "grep", "-n", "--no-color", "-wE", term],
    }

    print(f"## {slug} ({lang})\n")
    print(
        f"**Init:** `{shlex_fmt(init_cmd)}` → {fmt_ms(init_ms)} ms "
        f"· {entities} entities, {refs} refs across {files} files"
        + (" ⚠ failed" if init_failed else "")
    )
    print()

    if entities == 0:
        print("_skipping queries — index was not built_\n")
        return

    print("| Query | Tool | Lines | Bytes | Median ms | Speedup | Bytes saved |")
    print("|---|---|---:|---:|---:|---:|---:|")
    for key, label, _ in queries:
        sb, sl, st, sf = run(sigil_cmds[key])
        gb, gl, gt, gf = run(grep_cmds[key], cwd=repo)
        speedup = fmt_ratio(st, gt) if st > 0 else "n/a"
        saved = fmt_ratio(sb, gb) if sb > 0 else "n/a"
        sigil_tag = "sigil" + (" ⚠" if sf else "")
        grep_tag = "git grep" + (" ⚠" if gf else "")
        print(
            f"| {label} | {sigil_tag} | {sl} | {sb} | {fmt_ms(st)} | — | — |"
        )
        print(
            f"| {label} | {grep_tag} | {gl} | {gb} | {fmt_ms(gt)} | {speedup} | {saved} |"
        )
    print()


def shlex_fmt(cmd):
    import shlex

    return " ".join(shlex.quote(c) for c in cmd)


def main():
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        sys.exit(1)
    corpus_tsv = Path(sys.argv[1])
    sigil_bin = Path.cwd() / "target/release/sigil"
    if not sigil_bin.exists():
        sigil_bin = Path.cwd() / "target/debug/sigil"

    rows = []
    for raw in corpus_tsv.read_text().splitlines():
        raw = raw.strip()
        if not raw or raw.startswith("#"):
            continue
        parts = raw.split("\t")
        if len(parts) < 6:
            print(f"skip (<6 cols): {raw}", file=sys.stderr)
            continue
        rows.append(
            {
                "slug": parts[0],
                "repo": parts[1],
                "language": parts[2],
                "symbol": parts[3],
                "file": parts[4],
                "term": parts[5],
            }
        )

    print(f"# sigil multi-language benchmark — {time.strftime('%Y-%m-%d')}\n")
    print(f"sigil binary: `{sigil_bin}`")
    print(f"git: `{shutil.which('git') or '?'}`\n")
    print(
        "Each repo: clean-slate `sigil index --no-rank` (cold-start), then "
        "three query shapes run 3× each (median ms), sigil vs `git grep`. "
        "Speedup/bytes-saved columns on the grep row are `grep / sigil` "
        "ratios — higher means sigil more compact/faster.\n"
    )

    for row in rows:
        if not Path(row["repo"]).exists():
            print(f"_skip {row['slug']} — repo missing at {row['repo']}_\n")
            continue
        run_bench(row, sigil_bin)


if __name__ == "__main__":
    main()
