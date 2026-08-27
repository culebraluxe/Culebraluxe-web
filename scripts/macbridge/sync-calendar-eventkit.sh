#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# MAC-SYNC-CAL-02 — EventKit snapshot runner (manual AND LaunchAgent entry).
#
# A boring, dependable Mac-side operational wrapper around the EXISTING EventKit
# bridge (scripts/macbridge/CalendarEventKit.swift, also `pnpm calendar:sync`).
# It:
#   1. resolves the repository root (launchd provides it via CULEBRALUXE_REPO)
#   2. runs the bridge with a bounded window and writes the JSON snapshot
#   3. appends one lightweight, timestamped status line per invocation
#      (attempted-at, result, snapshot generated-at, event count)
#   4. is NON-FATAL: EventKit permission denial / missing swift / a bad run
#      only logs a failure and exits non-zero — the web app's Catch-Up adapter
#      already degrades gracefully when the snapshot is missing/bad.
#
# Env controls (all optional):
#   CULEBRALUXE_REPO           - repository root (set by the deployed copy)
#   MAC_BRIDGE_CALENDAR_JSON   - snapshot path (default /tmp/culebraluxe-calendar.json)
#   CALENDAR_SYNC_PAST_DAYS    - bridge look-back window (default 7)
#   CALENDAR_SYNC_FUTURE_DAYS  - bridge look-ahead window (default 60)
#   CULEBRALUXE_CALENDAR_LOG_DIR - where the invocation log lives
# ---------------------------------------------------------------------------

set -uo pipefail

if [ -n "${CULEBRALUXE_REPO:-}" ]; then
  REPO_ROOT="$CULEBRALUXE_REPO"
else
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd -P)"
fi

SNAPSHOT="${MAC_BRIDGE_CALENDAR_JSON:-/tmp/culebraluxe-calendar.json}"
PAST_DAYS="${CALENDAR_SYNC_PAST_DAYS:-7}"
FUTURE_DAYS="${CALENDAR_SYNC_FUTURE_DAYS:-60}"

LOG_DIR="${CULEBRALUXE_CALENDAR_LOG_DIR:-$HOME/Library/Logs/CulebraLuxe}"
LOG_FILE="$LOG_DIR/calendar-sync.invocations.log"

mkdir -p "$LOG_DIR" || { echo "cannot create log dir: $LOG_DIR" >&2; exit 1; }

stamp() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
attempted_at="$(stamp)"
log() { printf '%s\n' "$*" >>"$LOG_FILE"; }

log "attempted-at=$attempted_at"

if ! command -v swift >/dev/null 2>&1; then
  log "result=failure reason=swift-not-found attempted-at=$attempted_at"
  exit 1
fi

before_mtime=""
if [ -f "$SNAPSHOT" ]; then
  before_mtime="$(stat -f '%m' "$SNAPSHOT" 2>/dev/null || true)"
fi

cd "$REPO_ROOT" || { log "result=failure reason=cannot-cd repo=$REPO_ROOT"; exit 1; }

if swift scripts/macbridge/CalendarEventKit.swift \
     --out "$SNAPSHOT" \
     --past-days "$PAST_DAYS" \
     --future-days "$FUTURE_DAYS" >>"$LOG_FILE" 2>&1; then
  after_mtime="$(stat -f '%m' "$SNAPSHOT" 2>/dev/null || echo '')"
  generated_at="$(date -u -r "$after_mtime" '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || echo "$after_mtime")"
  count="$(node -e "try{const a=JSON.parse(require('fs').readFileSync(process.argv[1],'utf8'));console.log(Array.isArray(a)?a.length:'?')}catch{console.log('?')}" "$SNAPSHOT" 2>/dev/null || echo '?')"
  changed="no"
  if [ -n "$before_mtime" ] && [ "$before_mtime" != "$after_mtime" ]; then
    changed="yes"
  fi
  log "result=success snapshot=$SNAPSHOT generated-at=$generated_at events=$count changed=$changed"
  exit 0
else
  log "result=failure snapshot=$SNAPSHOT attempted-at=$attempted_at"
  exit 1
fi
