#!/usr/bin/env python3
"""Which of the 465 legacy test files already have an equivalent Rust test?

The conversion queue is only honest if it shrinks when work lands. This scans the Rust tree for
tests that already pin the subject of a legacy file, and writes the evidence to
`docs/agent/legacy-test-parity/already-covered.tsv`, which `scripts/legacy-test-parity.sh` folds
into the ledger.

Evidence, strongest first - the tier is written down because a guess must never remove a file from
the queue:

  citation      a Rust source cites the legacy file path itself (e.g. a `//! PORTED FROM` header).
                Strongest: whoever ported it said so.
  symbol        the legacy file's imported symbols resolve to Rust definitions in a file that has
                `#[test]`. This proves a Rust test exists for the symbol; it does NOT prove every
                assertion of the legacy file is reproduced, so these rows are marked
                `already_covered_verify` in the ledger: out of the conversion queue, on the list to
                spot-check.
  name_overlap  only the test-case names line up (>= 5 shared significant tokens, Jaccard >= 0.7).
                Never removes a file from the queue; the ledger shows it as `check_me`.

What it cannot see: a capability living under a different name with different words, and a renamed
symbol. Those stay `unassessed` and in the queue, which is the safe direction.
"""
from __future__ import annotations

import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEGACY = os.path.join(ROOT, "legacy")
RUST = os.path.join(ROOT, "rust")
OUT = os.path.join(ROOT, "docs/agent/legacy-test-parity/already-covered.tsv")

STOP = {
    "the", "a", "an", "and", "or", "of", "to", "in", "is", "it", "its", "for", "on", "by", "with",
    "that", "this", "not", "no", "does", "do", "are", "as", "at", "from", "must", "can", "so",
    "test", "tests", "s", "t", "be", "if", "when", "than", "then", "only", "but", "into", "up",
    "out", "one", "two", "every", "each", "still", "own", "same", "way", "over", "under",
}

CAMEL = re.compile(r"(?<=[a-z0-9])(?=[A-Z])")
TEST_ATTR = re.compile(r"#\[(?:tokio::)?test(?:\([^\]]*\))?\]\s*(?:async\s+)?fn\s+(\w+)")
DEF_RE = re.compile(
    r"\b(?:pub(?:\([^)]*\))?\s+)?(?:fn|const|static|struct|enum|trait|type)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
IMPORT_RE = re.compile(r"import\s*(?:type\s*)?\{([^}]*)\}\s*from\s*['\"]([^'\"]+)['\"]", re.S)
CASE_RE = re.compile(r"\b(?:test|it)\(\s*['\"]([^'\"]{3,120})['\"]")


def snake(name: str) -> str:
    """camelCase / PascalCase / SCREAMING_SNAKE -> one comparable snake_case form."""
    n = name.strip().rstrip("?")
    if n.isupper():
        return n.lower()
    return CAMEL.sub("_", n).lower()


def tokens(text: str) -> set[str]:
    words = re.findall(r"[a-z0-9]+", text.lower())
    return {w for w in words if w not in STOP and len(w) > 2}


def rust_files() -> list[str]:
    out = []
    for dirpath, dirnames, filenames in os.walk(RUST):
        dirnames[:] = [d for d in dirnames if d not in {"target", "experiments"}]
        for f in filenames:
            if f.endswith(".rs"):
                out.append(os.path.join(dirpath, f))
    return sorted(out)


def main() -> int:
    defs: dict[str, list[tuple[str, int]]] = defaultdict(list)   # symbol -> (rust file, line)
    tests: dict[str, list[tuple[str, str]]] = defaultdict(list)  # rust file -> [(test fn, site)]
    all_tests: list[tuple[str, str]] = []                        # (test name, rust file)
    citations: dict[str, list[str]] = defaultdict(list)          # legacy path/basename -> rust file

    for path in rust_files():
        src = open(path, encoding="utf-8", errors="replace").read()
        rel = os.path.relpath(path, ROOT)
        for i, line in enumerate(src.splitlines(), 1):
            for m in DEF_RE.finditer(line):
                defs[snake(m.group(1))].append((rel, i))
        for m in TEST_ATTR.finditer(src):
            line = src[: m.start()].count("\n") + 1
            tests[rel].append((m.group(1), f"{rel}:{line}"))
            all_tests.append((m.group(1), rel))
        for m in re.finditer(r"legacy/[A-Za-z0-9_./-]+", src):
            # Provenance is written in comments. A path handed to a function - `section_for_file(
            # "legacy/workflow_app/tests/claim-clock.test.ts")` - is a routing table entry, and
            # citing a file for routing proves nothing about its assertions.
            line_start = src.rfind("\n", 0, m.start()) + 1
            prefix = src[line_start : m.start()]
            if "//" not in prefix:
                continue
            cited = m.group(0)
            citations[os.path.basename(cited)].append(rel)
            citations[cited].append(rel)

    legacy_files = []
    for dirpath, _, filenames in os.walk(LEGACY):
        for f in filenames:
            if f.endswith((".test.ts", ".test.tsx")):
                legacy_files.append(os.path.join(dirpath, f))
    legacy_files.sort()

    rows = []  # name_overlap candidates are printed, never queued
    counted = defaultdict(int)
    meta = {}  # path -> (tier, status, evidence, note)

    def test_count(files: list[str]) -> int:
        """Tests that live in the evidence file(s) - the size of what already exists."""
        seen = 0
        for f in files:
            seen += len(tests.get(f, []))
        return seen

    def record(rel: str, tier: str, ev: str, note: str) -> None:
        """already_covered only when the evidence spans the file's cases; otherwise gap.

        With `case_overlap` present the question is per case: how many of the legacy file's cases
        have a Rust test about that same thing. Without it (symbol tier) the weaker count test
        applies, and the ledger calls the row `gap` unless the counts clear it.
        """
        m = re.search(r"rust_tests=(\d+) legacy_cases=(\d+)", note)
        rust_n, case_n = int(m.group(1)), int(m.group(2))
        hit = re.search(r"case_overlap=(\d+)", note)
        spans = bool(hit) and case_n and int(hit.group(1)) >= case_n
        meta[rel] = (tier, "already_covered" if spans else "gap", ev, note)

    def overlap_cases(case_texts: list[str], files: list[str]) -> int:
        """How many legacy cases have a Rust test in `files` sharing >= 2 significant words.

        A count of tests is not coverage: a file of 16 unrelated tests says nothing about a 16-case
        legacy file. This asks the narrower question - is there a Rust test that talks about the
        same thing as this case - and it is what a citation has to clear before it may remove a file
        from the queue.
        """
        have = [tokens(name) for name, file in all_tests if file in files]
        hits = 0
        for case in case_texts:
            want = tokens(case)
            if any(len(want & h) >= 2 for h in have if h):
                hits += 1
        return hits

    for path in legacy_files:
        rel = os.path.relpath(path, ROOT)
        src = open(path, encoding="utf-8", errors="replace").read()
        base = os.path.basename(path)
        case_texts = CASE_RE.findall(src)
        cases = len(case_texts)

        tier = ""
        ev_files: list[str] = []

        cited = sorted(set(citations.get(base, []) + citations.get(rel, [])))
        if cited:
            tier, ev_files = "citation", cited
        else:
            symbols = []
            for names, _mod in IMPORT_RE.findall(src):
                for raw in names.split(","):
                    raw = raw.strip().split(" as ")[-1].strip()
                    if raw and raw != "type" and re.match(r"^[A-Za-z_]", raw):
                        symbols.append(raw)
            symbols = list(dict.fromkeys(symbols))
            hit_files: set[str] = set()
            matched = 0
            for sym in symbols:
                for file, _line in defs.get(snake(sym), []):
                    if tests.get(file):
                        hit_files.add(file)
                        matched += 1
                        break
            if symbols and matched / len(symbols) >= 0.6:
                tier = "symbol"
                ev_files = sorted(hit_files)

        if ev_files:
            hits = overlap_cases(case_texts, ev_files)
            if hits:
                note = f"rust_tests={test_count(ev_files)} legacy_cases={cases} case_overlap={hits}"
                if tier == "symbol":
                    note = f"{len(ev_files)} file(s); " + note
                record(rel, tier, ", ".join(ev_files[:3]), note)
                counted[tier] += 1
                continue
            # A Rust test exists for the subject but none of this file's cases: keep it queued.
            counted["no_case_overlap"] += 1
            continue

        if cases:
            want_all: set[str] = set()
            for c in CASE_RE.findall(src):
                want_all |= tokens(c)
            best = (0.0, "", [])
            for name, file in all_tests:
                have = tokens(name)
                if not have:
                    continue
                shared = want_all & have
                if len(shared) < 5:
                    continue
                jac = len(shared) / len(want_all | have)
                if jac > best[0]:
                    best = (jac, file, sorted(shared))
            if best[0] >= 0.7:
                rows.append(
                    (rel, "name_overlap", best[1], f"jaccard={best[0]:.2f} shared={','.join(best[2][:6])}")
                )
                counted["name_overlap"] += 1
                continue

        counted["none"] += 1

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write("# generated by scripts/legacy-test-merge-scan.py - do not edit\n")
        fh.write("# path\ttier\tstatus\trust evidence\tnote\n")
        for rel in sorted(meta):
            tier, status, ev, note = meta[rel]
            fh.write("\t".join((rel, tier, status, ev, note)) + "\n")

    full = sum(1 for v in meta.values() if v[1] == "already_covered")
    partial = sum(1 for v in meta.values() if v[1] == "gap")
    covered_cases = 0
    for note in (v[3] for v in meta.values()):
        m = re.search(r"legacy_cases=(\d+)", note)
        if m:
            covered_cases += int(m.group(1))

    print(f"scanned {len(legacy_files)} legacy test files against {len(all_tests)} Rust tests")
    print(f"  evidence of an existing Rust test: {len(meta)} files "
          f"({counted['citation']} cited by a Rust source, {counted['symbol']} symbol-level)")
    print(f"    already_covered (every case has a Rust test): {full}  -> out of the queue")
    print(f"    gap            (part of the file is covered): {partial}  -> out of the queue, delta recorded")
    print(f"    legacy cases in those files: {covered_cases}")
    print(f"  Rust test exists but no case matches (stays in queue): {counted['no_case_overlap']}")
    print(f"  name_overlap only (stays in queue): {counted['name_overlap']}")
    print(f"  no evidence found (stays in queue): {counted['none']}")
    print(f"wrote {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
