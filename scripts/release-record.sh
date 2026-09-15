#!/usr/bin/env bash
set -euo pipefail

# MASTER RELEASE — build, deploy, probe, and RECORD what actually happened.
#
#   pnpm release                build + deploy + probe + record
#   pnpm release --build        build + record only
#   pnpm release --deploy       deploy + probe + record only
#   pnpm release --probe        probe only (live re-check of the current SHA)
#   pnpm release --last [N]     print the last N COMPLETE records (default 10)
#   pnpm release --verify <sha> answer "is there an eligible receipt for this SHA?"
#
# The two scripts stay, for build-only and deploy-only work. The Forge chain never builds and never deploys:
# it asks --verify and reads the answer.
#
# A ROW IS NOT A RECEIPT. Grok, 2026-09-15, named the three ways a receipt lies:
#   1. a STALE cite — a receipt for SHA A read as evidence for candidate B;
#   2. a PARTIAL row — recorded without a live probe agreeing that anything is actually serving;
#   3. a LAST-LINE RACE — a reader consuming a half-written final line of the append-only file.
# So: a receipt is ELIGIBLE only for the SHA it measured, only when build, deploy and a live probe all agree
# on that same SHA, and a torn line is never read as a row. Everything else is recorded honestly — a failed
# release is a row too, because a missing row is not evidence of a clean release.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

RECORD="${RELEASE_RECORD_FILE:-docs/agent/releases.md}"
MODE="all"
LAST_N="10"
VERIFY_SHA=""

case "${1:-}" in
  --build) MODE="build" ;;
  --deploy) MODE="deploy" ;;
  --probe) MODE="probe" ;;
  --last) MODE="last"; LAST_N="${2:-10}" ;;
  --verify) MODE="verify"; VERIFY_SHA="${2:-}" ;;
  "" ) ;;
  *) printf 'usage: %s [--build|--deploy|--probe|--last [N]|--verify <sha>]\n' "$0" >&2; exit 2 ;;
esac

# A row has 10 columns, so a marked-up row line splits into 12 fields on '|'. The header, the separator and
# the prose are not rows, and a TORN last line has fewer fields — the reader drops it rather than reading a
# half-written append as a receipt.
complete_rows() {
  [ -f "$RECORD" ] || return 0
  awk -F'|' 'NF==12 { print }' "$RECORD"
}

if [ "$MODE" = "last" ]; then
  if [ ! -f "$RECORD" ]; then
    printf 'no releases recorded yet (%s does not exist)\n' "$RECORD"
    exit 0
  fi
  printf 'LAST %s COMPLETE RELEASE ROWS — %s\n\n' "$LAST_N" "$RECORD"
  complete_rows | tail -n "$LAST_N" || true
  exit 0
fi

if [ "$MODE" = "verify" ]; then
  [ -n "$VERIFY_SHA" ] || { printf 'ERROR: --verify needs a sha\n' >&2; exit 2; }
  # ELIGIBLE means: build, deploy and probe all zero, AND the row's sha is the sha asked about. A short sha
  # matches its full form; nothing else does.
  if complete_rows | awk -F'|' -v want="$VERIFY_SHA" '
        { sha=$4; gsub(/[ `]/, "", sha); b=$7; d=$8; p=$9; e=$10
          gsub(/ /, "", b); gsub(/ /, "", d); gsub(/ /, "", p); gsub(/ /, "", e)
          if ((sha == want || index(want, sha) == 1 || index(sha, want) == 1) && b == "0" && d == "0" && p == "0" && e == "yes") found=1 }
        END { exit found ? 0 : 1 }'; then
    printf 'ELIGIBLE — a receipt exists for %s (build, deploy and probe all agree)\n' "$VERIFY_SHA"
    exit 0
  fi
  printf 'NOT ELIGIBLE — no receipt for %s with a passing build, deploy and live probe\n' "$VERIFY_SHA"
  exit 1
fi

command -v git >/dev/null 2>&1 || { printf 'ERROR: git is required\n' >&2; exit 1; }

# The facts are read BEFORE anything runs: a release that dies half way still records what it attempted.
BUILD_SHA="$(printf '%.12s' "$(git rev-parse HEAD)")"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
DIRTY="clean"; [ -n "$(git status --porcelain)" ] && DIRTY="dirty"
STARTED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

printf '\n=== MASTER RELEASE ===\n  %s on %s (%s)\n  mode: %s\n\n' "$BUILD_SHA" "$BRANCH" "$DIRTY" "$MODE"

BUILD_RC="skipped"; DEPLOY_RC="skipped"; PROBE_RC="skipped"; DEPLOY_SHA="$BUILD_SHA"

# Never `set -e` through a wrapped script: its exit code IS the record.
set +e
if [ "$MODE" = "all" ] || [ "$MODE" = "build" ]; then
  printf -- '--- build ---\n'; bash scripts/vercel-build-prod.sh; BUILD_RC="$?"
  printf -- '--- build exit: %s ---\n' "$BUILD_RC"
fi
if [ "$MODE" = "all" ] || [ "$MODE" = "deploy" ]; then
  if [ "$BUILD_RC" = "skipped" ] || [ "$BUILD_RC" = "0" ]; then
    # Re-read the sha: if the tree moved between build and deploy, this row can never be a receipt for the
    # built sha — Grok's stale cite caught at the source instead of discovered later.
    DEPLOY_SHA="$(printf '%.12s' "$(git rev-parse HEAD)")"
    printf -- '--- deploy ---\n'; bash scripts/vercel-deploy-prod.sh; DEPLOY_RC="$?"
    printf -- '--- deploy exit: %s ---\n' "$DEPLOY_RC"
  else
    DEPLOY_RC="blocked-by-build"; printf -- '--- deploy skipped: the build failed ---\n'
  fi
fi
if [ "$MODE" = "all" ] || [ "$MODE" = "deploy" ] || [ "$MODE" = "probe" ]; then
  # THE LIVE PROBE. It defaults to the production host the deploy script itself uses, because a default that
  # is not a real check would make every row ineligible while looking configured:
  #   * RELEASE_PROBE_CMD unset  -> probe the prod host's root and require 200
  #   * RELEASE_PROBE_CMD=skip   -> recorded as skipped, never eligible (the honest opt-out)
  #   * RELEASE_PROBE_CMD=<cmd>  -> your check; its exit code is the record's
  PROBE_URL="${CULEBRALUXE_PROD_URL:-https://www.culebraluxe.com}"
  PROBE_CMD="${RELEASE_PROBE_CMD:-curl -fsS -o /dev/null -w '%{http_code}' \"$PROBE_URL\" | grep -q 200}"
  if [ "$PROBE_CMD" = "skip" ]; then
    printf -- '--- probe skipped by request (this row can never be an eligible receipt) ---\n'
  else
    printf -- '--- probe: %s ---\n' "$PROBE_CMD"
    bash -c "$PROBE_CMD"; PROBE_RC="$?"
    printf -- '--- probe exit: %s ---\n' "$PROBE_RC"
  fi
fi
set -e

ENDED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SHA_NOTE=""; [ "$BUILD_SHA" != "$DEPLOY_SHA" ] && SHA_NOTE="FAILED_SHA_MOVED_BETWEEN_BUILD_AND_DEPLOY"
ELIGIBLE="no"; OUTCOME="ok"
if [ "$BUILD_RC" != "skipped" ] && [ "$BUILD_RC" != "0" ]; then OUTCOME="BUILD_FAILED"; fi
if [ "$DEPLOY_RC" != "skipped" ] && [ "$DEPLOY_RC" != "0" ] && [ "$DEPLOY_RC" != "blocked-by-build" ]; then
  OUTCOME="DEPLOY_FAILED"
fi
if [ "$PROBE_RC" != "0" ] && [ "$PROBE_RC" != "skipped" ]; then OUTCOME="PROBE_FAILED"; fi
if [ "$BUILD_RC" = "0" ] && [ "$DEPLOY_RC" = "0" ] && [ "$PROBE_RC" = "0" ] && [ -z "$SHA_NOTE" ]; then
  ELIGIBLE="yes"
else
  [ "$OUTCOME" = "ok" ] && OUTCOME="NOT_ELIGIBLE"
  [ -n "$SHA_NOTE" ] && OUTCOME="$SHA_NOTE"
fi

if [ ! -f "$RECORD" ]; then
  cat >"$RECORD" <<'HEADER'
# Releases — what was actually built, deployed and probed

Append-only, written by `pnpm release` (`scripts/release-record.sh`). One row per attempt, failures included:
a missing row is not evidence of a clean release. `eligible=yes` means build, deploy and a live probe all
passed **for the sha in this row** — that, and only that, is a receipt. Everything else is a record.

| started (UTC) | ended (UTC) | sha | branch | tree | build | deploy | probe | eligible | outcome |
|---|---|---|---|---|---|---|---|---|---|
HEADER
fi

printf '| %s | %s | `%s` | %s | %s | %s | %s | %s | %s | %s |\n' \
  "$STARTED" "$ENDED" "$BUILD_SHA" "$BRANCH" "$DIRTY" "$BUILD_RC" "$DEPLOY_RC" "$PROBE_RC" "$ELIGIBLE" "$OUTCOME" >>"$RECORD"

printf '\nrecorded in %s (eligible=%s, outcome=%s):\n' "$RECORD" "$ELIGIBLE" "$OUTCOME"
complete_rows | tail -n 1

git add "$RECORD" 2>/dev/null || true
if ! git diff --cached --quiet 2>/dev/null; then
  git commit -q -m "release: record ${BUILD_SHA} (eligible=${ELIGIBLE}, ${OUTCOME})" -- "$RECORD" || true
  printf 'record committed\n'
fi

[ "$ELIGIBLE" = "yes" ] || exit 1

