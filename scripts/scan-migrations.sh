#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scan-migrations — squawk over the SQL migrations.
#
# WHY CHANGED-ONLY BY DEFAULT: the 192 migrations already applied cannot be un-applied, so linting
# them is archaeology. What matters is the statement about to hit a live database. `--all` prints
# the historical profile (745 findings on 2026-09-18: 145 indexes created without CONCURRENTLY,
# 50 constraints added without NOT VALID, 15 foreign keys, 2 NOT NULL columns) and is for review,
# not for a gate — a gate that is red on day one is a gate everyone learns to ignore.
#
# Exits 1 when squawk reports findings, so it can be a gate without a wrapper.
# ---------------------------------------------------------------------------
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! command -v squawk >/dev/null 2>&1; then
  echo 'scan:migrations: squawk not installed — npm install -g squawk-cli' >&2
  exit 2
fi

files=()

if [[ "${1:-}" == "--all" ]]; then
  for f in db/migrations/*.sql; do
    [[ -e "$f" ]] && files+=("$f")
  done
else
  # Committed range (the CI case): BASE_REF defaults to origin/main.
  base="${BASE_REF:-origin/main}"
  if git rev-parse --verify --quiet "$base" >/dev/null; then
    while IFS= read -r f; do
      [[ -n "$f" && -e "$f" ]] && files+=("$f")
    done < <(git diff --name-only --diff-filter=ACM "$base...HEAD" -- 'db/migrations/*.sql')
  fi
  # Uncommitted and untracked migrations (the local case: nothing committed yet).
  while IFS= read -r f; do
    [[ -n "$f" && -e "$f" ]] && files+=("$f")
  done < <(git status --porcelain -- 'db/migrations/*.sql' | awk '{print $NF}')
fi

# De-duplicate while keeping it bash 3.2 safe (macOS ships bash 3.2: no mapfile, no associative arrays).
unique=()
for f in "${files[@]:-}"; do
  [[ -z "$f" ]] && continue
  seen=0
  for u in "${unique[@]:-}"; do
    [[ "$u" == "$f" ]] && seen=1 && break
  done
  [[ $seen -eq 0 ]] && unique+=("$f")
done

if [[ ${#unique[@]} -eq 0 ]]; then
  echo 'scan:migrations: no changed migrations — nothing to lint'
  exit 0
fi

echo "scan:migrations: ${#unique[@]} changed migration file(s)"
squawk "${unique[@]}" --reporter tty
status=$?

if [[ $status -ne 0 ]]; then
  echo ''
  echo 'scan:migrations: squawk reported the findings above. Each one is a statement that can block'
  echo 'or lock a live table. Fix the statement (CONCURRENTLY, NOT VALID, a separate backfill) or'
  echo 'record why it is safe in the migration file itself.'
fi
exit $status
