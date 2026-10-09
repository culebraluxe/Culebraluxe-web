#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# SHELL SYNTAX GATE — `bash -n` over every tracked shell file. A PARSE, not a lint.
#
# WHY THIS EXISTS (2026-10-08). `scripts/release-ci-check.sh` — the file that decides whether a release may
# proceed — was found carrying a live syntax error: an apostrophe inside a bash single-quoted `node -e`
# block, left there by an edit that nothing ever parsed. Nothing in the estate syntax-checked a shell
# script: not `.githooks/pre-push`, not `pnpm slice:check`, not `gates.yml`. A shell file that cannot be
# parsed cannot run, and a release-path script that cannot run fails at the worst possible moment. So the
# cheapest useful check was missing entirely, and this is it: read the file, report the line, stop.
#
# WHAT IT IS NOT: it does not run anything, does not import shellcheck's opinions (those are style, and a
# gate that argues about style gets deleted), and has no allow-list — a file that does not parse is a
# defect with a line number, not a finding to triage.
#
# SCOPED, stated so the silence is not mistaken for coverage: tracked `*.sh` and tracked files under
# `.githooks/`. `legacy/` (the retired TypeScript estate, marked dead on purpose) and `experiments/`
# (comparison benches, outside the workspace) are excluded — neither is a live gate's business.
# ---------------------------------------------------------------------------
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2

checked=0
failed=0

while IFS= read -r file; do
  [ -n "$file" ] || continue
  checked=$((checked + 1))
  if ! out="$(bash -n "$file" 2>&1)"; then
    failed=$((failed + 1))
    printf 'shell-syntax: FAIL %s\n' "$file"
    printf '%s\n' "$out" | sed 's/^/    /'
  fi
done < <(git ls-files '*.sh' '.githooks/*' | grep -v '^legacy/' | grep -v '^experiments/' || true)

if [ "$failed" -gt 0 ]; then
  printf 'shell-syntax: %d of %d tracked shell file(s) do not parse — see above\n' "$failed" "$checked" >&2
  exit 1
fi

printf 'shell-syntax: %d tracked shell file(s) parse cleanly\n' "$checked"
