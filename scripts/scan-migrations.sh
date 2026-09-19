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
#
# JSON MODE (`--json`): stdout is exactly one JSON object
#   {"files":[...],"findings":[{"file","rule","line","message",...}]}
# and all human commentary goes to stderr. Exit 0 clean/no-change, 1 findings, 2 when migration
# files were selected but squawk is not installed (fail closed — an unavailable linter is not a
# clean lint). Explicit file arguments after `--json` lint exactly those files, bypassing git, so
# the engine fence can drive a fixture without becoming a second changed-file writer.
# ---------------------------------------------------------------------------
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

json=0
all=0
explicit=()
for a in "$@"; do
  case "$a" in
    --json) json=1 ;;
    --all) all=1 ;;
    *) explicit+=("$a") ;;
  esac
done

files=()

if [[ ${#explicit[@]} -gt 0 ]]; then
  for f in "${explicit[@]}"; do
    [[ -n "$f" && -e "$f" ]] && files+=("$f")
  done
elif [[ $all -eq 1 ]]; then
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

# NO FILES IS NOT A FAILURE. This is checked BEFORE the tool so a checkout with no changed
# migrations never needs squawk installed, and unrelated work is never blocked by history.
if [[ ${#unique[@]} -eq 0 ]]; then
  if [[ $json -eq 1 ]]; then
    printf '{"files":[],"findings":[]}\n'
  else
    echo 'scan:migrations: no changed migrations — nothing to lint'
  fi
  exit 0
fi

if ! command -v squawk >/dev/null 2>&1; then
  echo 'scan:migrations: squawk not installed — npm install -g squawk-cli' >&2
  exit 2
fi

# EXCLUDED RULES, and what excluding them costs (operator decision, 2026-09-19).
#
# `changing-column-type` fires on every ALTER COLUMN ... TYPE because an ACCESS EXCLUSIVE rewrite on a live
# table blocks readers and writers. That is the right warning for a table with clients, and the wrong one for a
# LANDING table whose only load path is a truncate-and-replace: widening a column there cannot strand a client,
# and the alternative ways to express it are worse — DROP + ADD trips `ban-drop-column`, and neither is cleaner
# than the ALTER it replaces. The first migration that needed it was l_Regrid's coordinate precision (199 typed
# numeric(11,7), the export carries 8 decimals, so the values were being ROUNDED) found by verifying the load
# cell-by-cell (scripts/verify-l-regrid.ts).
#
# THE COST, STATED PLAINLY: this exclusion is GLOBAL, not per-file — a type change anywhere in a changed
# migration is no longer flagged. The rule that replaces it is review: a type change has to say in its own file
# why the rewrite is safe, which is what migration 200 does. Everything else squawk reports still fails the gate.
SQUAWK_EXCLUDES=(--exclude=changing-column-type)

if [[ $json -eq 1 ]]; then
  echo "scan:migrations: ${#unique[@]} migration file(s) selected" >&2
  files_json=""
  for f in "${unique[@]}"; do
    files_json+="\"$f\","
  done
  files_json="[${files_json%,}]"
  squawk_out="$(squawk "${unique[@]}" "${SQUAWK_EXCLUDES[@]}" --reporter json 2>/dev/null)"
  status=$?
  [[ -z "$squawk_out" ]] && squawk_out='[]'
  # Normalize squawk's native `rule_name` to the contract's `rule` so a caller reads one name.
  squawk_out="$(printf '%s' "$squawk_out" | sed 's/"rule_name":/"rule":/g')"
  printf '{"files":%s,"findings":%s}\n' "$files_json" "$squawk_out"
  exit $status
fi

echo "scan:migrations: ${#unique[@]} changed migration file(s)"
squawk "${unique[@]}" "${SQUAWK_EXCLUDES[@]}" --reporter tty
status=$?

if [[ $status -ne 0 ]]; then
  echo ''
  echo 'scan:migrations: squawk reported the findings above. Each one is a statement that can block'
  echo 'or lock a live table. Fix the statement (CONCURRENTLY, NOT VALID, a separate backfill) or'
  echo 'record why it is safe in the migration file itself.'
fi
exit $status
