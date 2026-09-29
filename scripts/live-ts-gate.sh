#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# LIVE TYPESCRIPT COUNT — a ratchet, not a number in a document.
#
# The 2026-09-28 review found the port unfinished: 69 TypeScript/JavaScript files outside `legacy/`
# still load and run, and every one of them is a candidate for the port. Nothing stopped a 70th from
# arriving, and `pnpm broken:ts:sweep` cannot see it: that sweep only demands a banner from files that
# CANNOT load (its own rule), so a new *working* script is invisible to it. A green sweep is not
# evidence that the TypeScript is gone. This gate counts the other half.
#
# What it counts: git-tracked `.ts/.tsx/.mts/.cts/.js/.mjs/.cjs` outside `legacy/`, not `*.d.ts`,
# that do not carry the `⚠ BROKEN ON PURPOSE` banner (first 12 lines, the same window
# `scripts/test-harness.mjs` uses).
#
# The count is 69 as of 2026-09-29, and the counting rule is why other numbers exist: a count that
# also includes the four excluded configuration files reads 73; a scan that treats a banner mention
# ANYWHERE in the file as a marker reads 71, because `scripts/broken-ts-sweep.mjs` and
# `scripts/dead-command-sweep.mjs` quote the banner in prose — and such a scan would forgive a new
# file that quotes it too. This window is the strict one on purpose.
#
# The rule — it may only FALL:
#   a live file that is not in the baseline   -> FAIL   (no new TypeScript)
#   a baselined file that is gone             -> FAIL   until the baseline is lowered (`--write`)
#   the two sets equal                        -> PASS   and the count is printed
# When the baseline reaches empty, this reads exactly what the work order asked for: a count of 0 or
# CI is red. Until then the count may only fall, and every run prints where it stands.
#
# `--write` lowers the baseline and REFUSES to raise it: removing a stale line is bookkeeping, adding
# TypeScript is a deliberate human act and is never a side effect of running the command that reports
# the number. `--list` groups the live set by tree.
#
# Two exclusions, both named rather than hidden:
#   * `legacy/` — the retired TypeScript, kept as reference by the repository's own decision.
#   * build configuration — `eslint.config.mjs`, `postcss.config.mjs`, `.dependency-cruiser*.js`:
#     not product code, not portable, they configure the tools that check what is left.
# Everything else in the tree is in scope, `scripts/` and `agent-runtime/` included.
#
# Usage: pnpm ts:count | pnpm ts:count:write | pnpm ts:count:list
# ---------------------------------------------------------------------------
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

BASELINE="docs/agent/live-ts-baseline.txt"
MARKER='BROKEN ON PURPOSE'
MARKER_LINES=12
CONFIG_RE='^(eslint\.config\.mjs|postcss\.config\.mjs|\.dependency-cruiser\.js|\.dependency-cruiser\.runtime\.js)$'

live_set() {
  git ls-files \
    | grep -E '\.(ts|tsx|mts|cts|js|mjs|cjs)$' \
    | grep -v '^legacy/' \
    | grep -v '\.d\.ts$' \
    | grep -Ev "$CONFIG_RE" \
    | while IFS= read -r file; do
        [ -f "$file" ] || continue
        head -n "$MARKER_LINES" "$file" | grep -q -- "$MARKER" && continue
        printf '%s\n' "$file"
      done \
    | sort -u
}

baseline_set() {
  [ -f "$BASELINE" ] || return 0
  grep -v '^[[:space:]]*#' "$BASELINE" | grep -v '^[[:space:]]*$' | sort -u
}

live="$(live_set)"
base="$(baseline_set)"
count="$(printf '%s\n' "$live" | grep -c . || true)"
new_files="$(comm -23 <(printf '%s\n' "$live") <(printf '%s\n' "$base"))"
gone_files="$(comm -13 <(printf '%s\n' "$live") <(printf '%s\n' "$base"))"

case "${1:-}" in
  --write)
    if [ -n "$new_files" ]; then
      echo "ts:count --write refuses to ADD TypeScript. These files are live and not in the baseline:"
      printf '%s\n' "$new_files" | sed 's/^/  /'
      echo
      echo "Port or retire them, or add them to $BASELINE deliberately with the reason in the commit message."
      exit 1
    fi
    {
      echo "# Live TypeScript outside legacy/ — the ratchet. This list may only FALL."
      echo "#"
      echo "# Read by scripts/live-ts-gate.sh; enforced by \`pnpm ts:count\` (CI job \`static gates\`)."
      echo "# Removed from this list because the file was ported, retired, or marked \`⚠ BROKEN ON PURPOSE\`."
      echo "# A file listed here that is gone is a FAILURE until this line is deleted: that is what makes"
      echo "# the count monotone. Never add a line to make a red gate green — port the file instead."
      echo "#"
      echo "# Count at the last write: $count (was $(printf '%s\n' "$base" | grep -c . || true) before this run)."
      printf '%s\n' "$live"
    } > "$BASELINE"
    echo "ts:count — baseline written: $count live file(s)"
    exit 0
    ;;
  --list)
    echo "ts:count — $count live file(s), grouped by tree:"
    printf '%s\n' "$live" | awk -F/ '{print (NF>1 ? $1 : "(root)")}' | sort | uniq -c | sort -rn | sed 's/^/  /'
    printf '%s\n' "$live" | sed 's/^/  /'
    exit 0
    ;;
esac

status=0
if [ -n "$new_files" ]; then
  status=1
  echo "ts:count FAIL — live TypeScript that is not in the baseline (new TypeScript is not allowed):"
  printf '%s\n' "$new_files" | sed 's/^/  + /'
  echo "  Port it to Rust (legacy/ is for reference; agent-runtime/ and scripts/ are for the engine and the harness),"
  echo "  or mark the file \`⚠ BROKEN ON PURPOSE\` if it cannot load, or add it to $BASELINE on purpose."
fi

if [ -n "$gone_files" ]; then
  status=1
  echo "ts:count FAIL — the baseline lists file(s) that no longer exist (lock the improvement in):"
  printf '%s\n' "$gone_files" | sed 's/^/  - /'
  echo "  Run \`pnpm ts:count:write\` to lower the baseline to $count."
fi

echo "ts:count — $count live TypeScript file(s) outside legacy/ (baseline: $(printf '%s\n' "$base" | grep -c . || true))"
if [ "$count" -eq 0 ]; then
  echo "ts:count — 0: the port's TypeScript is finished. This gate now fails on any single new file."
fi
exit "$status"
