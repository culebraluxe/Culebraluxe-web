#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# MAC-SYNC-CAL-02 — EventKit gateway runner (manual AND LaunchAgent entry).
#
# The Mac is the Apple Calendar edge. This wrapper:
#   1. resolves the repository root (launchd provides it via CULEBRALUXE_REPO)
#   2. runs the read-only EventKit bridge with a bounded window
#   3. lands that normalized snapshot into PROD l_calendar through the existing
#      replay-safe landing repository (source_account + source_message_id)
#   4. appends one lightweight, timestamped status line per invocation
#
# Apple Calendar remains authoritative. This job does not write back to EventKit.
#
# Env controls (all optional):
#   CULEBRALUXE_REPO             - repository root (set by the deployed copy)
#   MAC_BRIDGE_CALENDAR_JSON     - snapshot path (default /tmp/culebraluxe-calendar.json)
#   CALENDAR_SYNC_PAST_DAYS      - bridge look-back window (default 7)
#   CALENDAR_SYNC_FUTURE_DAYS    - bridge look-ahead window (default 60)
#   CULEBRALUXE_CALENDAR_LOG_DIR - where the invocation log lives
# ---------------------------------------------------------------------------

set -uo pipefail

# launchd starts jobs with a minimal PATH that does not include Homebrew.
# CulebraLuxe dev Macs are Apple Silicon today (/opt/homebrew/bin), while the
# /usr/local/bin fallback keeps the wrapper portable to Intel/Homebrew installs.
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:${PATH:-}"

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
if ! command -v node >/dev/null 2>&1; then
  log "result=failure reason=node-not-found attempted-at=$attempted_at"
  exit 1
fi
NODE_BIN="$(command -v node)"

before_mtime=""
if [ -f "$SNAPSHOT" ]; then
  before_mtime="$(stat -f '%m' "$SNAPSHOT" 2>/dev/null || true)"
fi

cd "$REPO_ROOT" || { log "result=failure reason=cannot-cd repo=$REPO_ROOT"; exit 1; }

if [ ! -f "$REPO_ROOT/.env.local" ]; then
  log "result=failure reason=missing-env-local path=$REPO_ROOT/.env.local"
  exit 1
fi

if ! swift scripts/macbridge/CalendarEventKit.swift \
     --out "$SNAPSHOT" \
     --past-days "$PAST_DAYS" \
     --future-days "$FUTURE_DAYS" >>"$LOG_FILE" 2>&1; then
  log "result=failure stage=eventkit snapshot=$SNAPSHOT attempted-at=$attempted_at"
  exit 1
fi

after_mtime="$(stat -f '%m' "$SNAPSHOT" 2>/dev/null || echo '')"
generated_at="$(date -u -r "$after_mtime" '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || echo "$after_mtime")"
count="$("$NODE_BIN" -e "try{const a=JSON.parse(require('fs').readFileSync(process.argv[1],'utf8'));console.log(Array.isArray(a)?a.length:'?')}catch{console.log('?')}" "$SNAPSHOT" 2>/dev/null || echo '?')"
changed="no"
if [ -n "$before_mtime" ] && [ "$before_mtime" != "$after_mtime" ]; then
  changed="yes"
fi

# The LaunchAgent is the production Apple Calendar gateway. The DB target is
# explicit and fail-closed; db/client resolves DATABASE_URL_PROD from .env.local.
if ! APP_ENV=production EXECUTION_ENV=PROD \
  "$NODE_BIN" --env-file="$REPO_ROOT/.env.local" --import tsx \
  scripts/calendar-eventkit-intake.ts "$SNAPSHOT" >>"$LOG_FILE" 2>&1; then
  log "result=failure stage=landing snapshot=$SNAPSHOT generated-at=$generated_at events=$count changed=$changed"
  exit 1
fi

log "result=success snapshot=$SNAPSHOT generated-at=$generated_at events=$count changed=$changed landed=prod"
exit 0
