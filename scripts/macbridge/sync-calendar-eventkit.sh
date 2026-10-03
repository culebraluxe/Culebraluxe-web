#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# MAC-SYNC-CAL-03 — Apple EventKit gateway runner (manual + LaunchAgent entry).
#
# One trusted Mac-side cycle handles the two Apple work flavors separately:
#   CulebraLuxe Calendar command -> EKEvent    -> l_calendar -> Schedule
#   CulebraLuxe WBS mirror       -> EKReminder -> l_reminder -> Work
#
# Outbound commands are drained FIRST. The same cycle then reads EventKit back
# into landing, giving us a real round trip instead of assuming Apple accepted
# the write. Canonical WBS remains canonical; Apple Calendar remains the source
# for generic schedule events.
# ---------------------------------------------------------------------------

set -uo pipefail

# launchd starts jobs with a minimal PATH that does not include Homebrew.
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:${PATH:-}"

if [ -n "${CULEBRALUXE_REPO:-}" ]; then
  REPO_ROOT="$CULEBRALUXE_REPO"
else
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd -P)"
fi

SNAPSHOT="${MAC_BRIDGE_CALENDAR_JSON:-/tmp/culebraluxe-calendar.json}"
REMINDERS_SNAPSHOT="${MAC_BRIDGE_REMINDERS_JSON:-/tmp/culebraluxe-reminders.json}"
PAST_DAYS="${CALENDAR_SYNC_PAST_DAYS:-7}"
FUTURE_DAYS="${CALENDAR_SYNC_FUTURE_DAYS:-60}"

# The shared machine log tree, outside every checkout so one directory answers "what did the jobs do"
# (docs/agent/LAYOUT.md, where the tree lives). `CULEBRALUXE_CALENDAR_LOG_DIR` (set by the plist) still wins.
LOG_DIR="${CULEBRALUXE_CALENDAR_LOG_DIR:-/Users/Shared/dev/build/logs}"
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
if ! command -v cargo >/dev/null 2>&1; then
  log "result=failure reason=cargo-not-found attempted-at=$attempted_at"
  exit 1
fi

before_mtime=""
if [ -f "$SNAPSHOT" ]; then
  before_mtime="$(stat -f '%m' "$SNAPSHOT" 2>/dev/null || true)"
fi

cd "$REPO_ROOT" || { log "result=failure reason=cannot-cd repo=$REPO_ROOT"; exit 1; }

if [ ! -f "$REPO_ROOT/.env.local" ]; then
  log "result=failure reason=missing-env-local path=$REPO_ROOT/.env.local"
  exit 1
fi

# --- Outbound: durable CulebraLuxe commands -> EventKit ----------------------
# The worker claims ONLY the two Apple subscriptions; it cannot steal unrelated
# FORGE/application MQ deliveries. It writes command payloads through private
# temp files and logs aggregate counts only.
if ! APP_ENV=production EXECUTION_ENV=PROD \
  cargo run --quiet --release --manifest-path "$REPO_ROOT/Cargo.toml" -p cli -- \
  apple-sync drain >>"$LOG_FILE" 2>&1; then
  log "result=failure stage=apple-outbound attempted-at=$attempted_at"
  exit 1
fi

# --- Schedule: EKEvent -------------------------------------------------------
if ! swift scripts/macbridge/CalendarEventKit.swift \
     --out "$SNAPSHOT" \
     --past-days "$PAST_DAYS" \
     --future-days "$FUTURE_DAYS" >>"$LOG_FILE" 2>&1; then
  log "result=failure stage=calendar-eventkit snapshot=$SNAPSHOT attempted-at=$attempted_at"
  exit 1
fi

# --- Work: EKReminder --------------------------------------------------------
if ! swift scripts/macbridge/RemindersEventKit.swift \
     --out "$REMINDERS_SNAPSHOT" >>"$LOG_FILE" 2>&1; then
  log "result=failure stage=reminders-eventkit snapshot=$REMINDERS_SNAPSHOT attempted-at=$attempted_at"
  exit 1
fi

after_mtime="$(stat -f '%m' "$SNAPSHOT" 2>/dev/null || echo '')"
generated_at="$(date -u -r "$after_mtime" '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || echo "$after_mtime")"
count="$(grep -c '"sourceMessageId"' "$SNAPSHOT" 2>/dev/null || echo '?')"
reminder_count="$(grep -c '"reminderIdentifier"' "$REMINDERS_SNAPSHOT" 2>/dev/null || echo '?')"
changed="no"
if [ -n "$before_mtime" ] && [ "$before_mtime" != "$after_mtime" ]; then
  changed="yes"
fi

# --- Inbound: EventKit snapshots -> PROD landing -----------------------------
if ! APP_ENV=production EXECUTION_ENV=PROD \
  cargo run --quiet --release --manifest-path "$REPO_ROOT/Cargo.toml" -p cli -- \
  apple-sync calendar-intake "$SNAPSHOT" >>"$LOG_FILE" 2>&1; then
  log "result=failure stage=calendar-landing snapshot=$SNAPSHOT generated-at=$generated_at events=$count changed=$changed"
  exit 1
fi

if ! APP_ENV=production EXECUTION_ENV=PROD \
  cargo run --quiet --release --manifest-path "$REPO_ROOT/Cargo.toml" -p cli -- \
  apple-sync reminder-intake "$REMINDERS_SNAPSHOT" >>"$LOG_FILE" 2>&1; then
  log "result=failure stage=reminder-landing snapshot=$REMINDERS_SNAPSHOT reminders=$reminder_count"
  exit 1
fi

log "result=success snapshot=$SNAPSHOT generated-at=$generated_at events=$count reminders=$reminder_count changed=$changed landed=prod"
exit 0
