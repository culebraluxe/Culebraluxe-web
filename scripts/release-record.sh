#!/usr/bin/env bash
set -euo pipefail

# MASTER RELEASE — build, deploy, and RECORD what actually happened.
#
# Why one command: with two scripts the evidence of a release lived only in the terminal that ran them, so
# "what is actually in production?" was unanswerable from this machine (2026-09-15 — the only honest answer
# was "my commits are on main" and nothing about whether they were live). The individual scripts STAY, for
# build-only or deploy-only work; this wraps them and writes the row.
#
#   pnpm release              build + deploy + record
#   pnpm release --build      build + record only
#   pnpm release --deploy     deploy + record only
#   pnpm release --last [N]   print the last N records (default 10)
#
# The record is append-only and local: docs/agent/releases.md. It is evidence, not ceremony — the Forge chain
# never builds and never deploys; its DEV_OPS step checks that a receipt exists for the SHA it is verifying.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

RECORD="docs/agent/releases.md"
MODE="all"
LAST_N="10"

case "${1:-}" in
  --build) MODE="build" ;;
  --deploy) MODE="deploy" ;;
  --last)
    MODE="last"
    LAST_N="${2:-10}"
    ;;
  "" ) ;;
  *)
    printf 'usage: %s [--build|--deploy|--last [N]]\n' "$0" >&2
    exit 2
    ;;
esac

if [ "$MODE" = "last" ]; then
  if [ ! -f "$RECORD" ]; then
    printf 'no releases recorded yet (%s does not exist)\n' "$RECORD"
    exit 0
  fi
  printf 'LAST %s RELEASES — %s\n\n' "$LAST_N" "$RECORD"
  grep '^| 20' "$RECORD" | tail -n "$LAST_N" || true
  exit 0
fi

command -v git >/dev/null 2>&1 || { printf 'ERROR: git is required\n' >&2; exit 1; }

# The facts are read BEFORE anything runs, so a release that dies half way still records what it attempted.
SHA_FULL="$(git rev-parse HEAD)"
SHA="$(printf '%.12s' "$SHA_FULL")"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
DIRTY="clean"; [ -n "$(git status --porcelain)" ] && DIRTY="dirty"
STARTED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

printf '\n=== MASTER RELEASE ===\n  %s on %s (%s)\n  mode: %s\n\n' "$SHA" "$BRANCH" "$DIRTY" "$MODE"

BUILD_RC="skipped"
DEPLOY_RC="skipped"

# Never `set -e` through a wrapped script: its exit code IS the record.
set +e
if [ "$MODE" = "all" ] || [ "$MODE" = "build" ]; then
  printf -- '--- build ---\n'
  bash scripts/vercel-build-prod.sh
  BUILD_RC="$?"
  printf -- '--- build exit: %s ---\n' "$BUILD_RC"
fi
if [ "$MODE" = "all" ] || [ "$MODE" = "deploy" ]; then
  if [ "$BUILD_RC" = "skipped" ] || [ "$BUILD_RC" = "0" ]; then
    printf -- '--- deploy ---\n'
    bash scripts/vercel-deploy-prod.sh
    DEPLOY_RC="$?"
    printf -- '--- deploy exit: %s ---\n' "$DEPLOY_RC"
  else
    DEPLOY_RC="blocked-by-build"
    printf -- '--- deploy skipped: the build failed ---\n'
  fi
fi
set -e

ENDED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
OUTCOME="ok"
if [ "$BUILD_RC" != "skipped" ] && [ "$BUILD_RC" != "0" ]; then OUTCOME="BUILD_FAILED"; fi
if [ "$DEPLOY_RC" != "skipped" ] && [ "$DEPLOY_RC" != "0" ] && [ "$DEPLOY_RC" != "blocked-by-build" ]; then
  OUTCOME="DEPLOY_FAILED"
fi

if [ ! -f "$RECORD" ]; then
  cat >"$RECORD" <<'HEADER'
# Releases — what was actually built and actually deployed

Append-only, written by `pnpm release` (`scripts/release-record.sh`). One row per release attempt, including
the failed ones: a missing row is not evidence of a clean release, so nothing is skipped or rewritten.

| started (UTC) | ended (UTC) | sha | branch | tree | build | deploy | outcome |
|---|---|---|---|---|---|---|---|
HEADER
fi

printf '| %s | %s | `%s` | %s | %s | %s | %s | %s |\n' \
  "$STARTED" "$ENDED" "$SHA" "$BRANCH" "$DIRTY" "$BUILD_RC" "$DEPLOY_RC" "$OUTCOME" >>"$RECORD"

printf '\nrecorded in %s:\n' "$RECORD"
tail -n 1 "$RECORD"

git add "$RECORD" 2>/dev/null || true
if ! git diff --cached --quiet 2>/dev/null; then
  git commit -q -m "release: record ${SHA} (${OUTCOME})" -- "$RECORD" || true
  printf 'record committed\n'
fi

[ "$OUTCOME" = "ok" ] || exit 1
