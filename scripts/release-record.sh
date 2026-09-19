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

# TWO QUESTIONS, NEVER ONE (Grok, 2026-09-15). `receiptFor(sha)` is what DEV_OPS may cite: an eligible row for
# THAT sha. `productionIs()` is what the live probe says NOW. A perfectly good old receipt answers the first and
# lies about the second, and that is why eligibility alone was not enough. An unset RELEASE_PROBE_CMD defaults
# to the production root the deploy script itself uses, because a default that is not a real check makes every
# row ineligible while looking configured; `skip` is the honest opt-out. Rows are never expired — history is
# not rewritten — so the freshness question is answered live instead.
PROBE_URL="${CULEBRALUXE_PROD_URL:-https://www.culebraluxe.com}"
PROBE_CMD="${RELEASE_PROBE_CMD:-curl -fsS -o /dev/null -w '%{http_code}' \"$PROBE_URL\" | grep -q 200}"
MODE="all"
LAST_N="10"
VERIFY_SHA=""

case "${1:-}" in
  --build) MODE="build" ;;
  --deploy) MODE="deploy" ;;
  --probe) MODE="probe" ;;
  --production) MODE="production" ;;
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
  # A ONE-CHARACTER PREFIX MATCHES EVERYTHING (Grok, 2026-09-16: `--verify 0` could hit a row). Seven
  # characters is the shortest git-unique-in-practice prefix; shorter is refused rather than answered.
  [ "${#VERIFY_SHA}" -ge 7 ] || {
    printf 'ERROR: --verify needs at least 7 characters of a sha (a shorter prefix matches too much)\n' >&2
    exit 2
  }
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

if [ "$MODE" = "production" ]; then
  # productionIs() — what production answers NOW. It makes NO claim about any sha: pairing a live answer with a
  # sha it did not measure is exactly the stale-but-self-consistent receipt, arrived at from the other side.
  if [ "$PROBE_CMD" = "skip" ]; then
    printf 'PRODUCTION NOW — probe disabled by request (no answer; this is not a pass)\n'
    exit 1
  fi
  printf 'PRODUCTION NOW — %s\n' "$PROBE_URL"
  printf '  probe: %s\n' "$PROBE_CMD"
  if bash -c "$PROBE_CMD"; then
    printf '  answer: SERVING\n  (this answers what is serving NOW; a receipt for a sha is --verify <sha>)\n'
    exit 0
  fi
  printf '  answer: NOT SERVING\n  (the live probe failed; any sha must be verified against a fresh release)\n'
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

# READ THE GATE BEFORE YOU BUILD (FORGE-LOCAL-RELEASE-CI-CHECK-01).
#
# A deploy job in CI was added and deleted for cost (it was doubling the bill): CI now CHECKS and never ships,
# and this is the deploy. That left a red main releasable by hand, because nothing here read the gate. The
# check below is a READ — `gh run list` for this exact sha — and it refuses on failed, pending, missing or
# unreadable results. RELEASE_CI_CHECK=skip is the named opt-out for an outage, and it says loudly that the
# gate was NOT read.
#
# HEAD IS RE-READ AFTER THE CHECK. A check that passes for one sha and a build that then ships another is
# the stale cite this file already guards against between build and deploy; here it is guarded between the
# check and the build, because the check is only worth its answer for the commit it was asked about.
if [ "$MODE" = "all" ] || [ "$MODE" = "build" ]; then
  CHECK_SHA="$(git rev-parse HEAD)"
  printf -- '--- release gate: CI results for %s ---\n' "$(printf '%.12s' "$CHECK_SHA")"
  bash scripts/release-ci-check.sh "$CHECK_SHA"; CI_CHECK_RC="$?"
  printf -- '--- release gate exit: %s ---\n' "$CI_CHECK_RC"
  AFTER_CHECK_SHA="$(git rev-parse HEAD)"
  if [ "$CI_CHECK_RC" -ne 0 ]; then
    printf '\n=== RELEASE REFUSED: CI is not green for %s (see above; RELEASE_CI_CHECK=skip to override) ===\n' \
      "$(printf '%.12s' "$CHECK_SHA")"
    exit 1
  fi
  if [ "$AFTER_CHECK_SHA" != "$CHECK_SHA" ]; then
    printf '\n=== RELEASE REFUSED: HEAD moved from %s to %s while the gate was being read ===\n' \
      "$(printf '%.12s' "$CHECK_SHA")" "$(printf '%.12s' "$AFTER_CHECK_SHA")"
    exit 1
  fi
  BUILD_SHA="$(printf '%.12s' "$CHECK_SHA")"
fi

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
  elif [ -n "${RELEASE_PROBE_CMD:-}" ]; then
    printf -- '--- probe (override): %s ---\n' "$PROBE_CMD"
    bash -c "$PROBE_CMD"; PROBE_RC="$?"
    printf -- '--- probe exit: %s ---\n' "$PROBE_RC"
  else
    # SHA-NAMED PROBE (Grok, 2026-09-16). A homepage 200 says "something is serving"; it does not say the sha
    # in this row is what is being served, which is the only thing an eligible receipt may claim.
    # /api/build-info is the deploy script's own sha probe (scripts/vercel-deploy-prod.sh:71), so the record
    # and the release agree by construction rather than by luck.
    LIVE_SHA="$(curl -fsS -L --max-time 25 "${PROBE_URL%/}/api/build-info" 2>/dev/null | sed -n 's/.*"sha"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' || true)"
    # THE STAMP IS SHORTER THAN THE ROW SHA (Grok, 2026-09-16). Production labels its build with a 7-character
    # stamp (cockpitBuildLabel) while a row records 12, so "live starts with the row sha" could never be true
    # and NO receipt could ever say yes. So compare the SHORTER of the two, from 7 characters up, in either
    # direction — still never a loose match: one character of agreement is not a match.
    sha_matches() {
      local a="$1" b="$2" n
      if [ "${#a}" -lt "${#b}" ]; then n="${#a}"; else n="${#b}"; fi
      [ "$n" -ge 7 ] || return 1
      [ "${a:0:$n}" = "${b:0:$n}" ]
    }
    if [ -n "$LIVE_SHA" ] && sha_matches "$LIVE_SHA" "$BUILD_SHA"; then
      PROBE_RC=0
      printf -- '--- probe: /api/build-info serves %s, this row measured %s ---\n' "$LIVE_SHA" "$BUILD_SHA"
    else
      PROBE_RC=1
      printf -- '--- probe FAILED: /api/build-info says %s, this row measured %s (not the same sha) ---\n' \
        "${LIVE_SHA:-<no answer>}" "$BUILD_SHA"
    fi
  fi
fi
set -e

ENDED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SHA_NOTE=""; [ "$BUILD_SHA" != "$DEPLOY_SHA" ] && SHA_NOTE="FAILED_SHA_MOVED_BETWEEN_BUILD_AND_DEPLOY"
ELIGIBLE="no"; OUTCOME="ok"
# THE FIRST FAILURE IS THE CAUSE; EVERY LATER ONE IS ITS CONSEQUENCE, AND THE CAUSE IS WHAT GETS NAMED.
# These were plain assignments, so the last step to fail won: a build failure left deploy blocked and the
# probe comparing the live host against a sha that never served, and the row read `PROBE_FAILED` for a
# release that never built (measured 2026-09-18, row 7697faa6d53c). Each branch now fires only while the
# outcome is still `ok`, so the row names the step that actually broke.
if [ "$OUTCOME" = "ok" ] && [ "$BUILD_RC" != "skipped" ] && [ "$BUILD_RC" != "0" ]; then OUTCOME="BUILD_FAILED"; fi
if [ "$OUTCOME" = "ok" ] && [ "$DEPLOY_RC" != "skipped" ] && [ "$DEPLOY_RC" != "0" ] && [ "$DEPLOY_RC" != "blocked-by-build" ]; then
  OUTCOME="DEPLOY_FAILED"
fi
if [ "$OUTCOME" = "ok" ] && [ "$PROBE_RC" != "0" ] && [ "$PROBE_RC" != "skipped" ]; then OUTCOME="PROBE_FAILED"; fi
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

