#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — the ONE ODS loader. ALWAYS PROD. DEV is never a target.
#
# Flavors:
#   contacts  (default, fast)  Apple Contacts, full field set
#                              export -> ODS staging -> l_person -> Person mastery
#                              -> names -> clients read models      (~1 minute)
#   imessage                   contacts + iMessage (the ~90k-message, ~3h load)
#   full                       contacts + iMessage + Apple calls + email
#
#   pnpm ods:load            # fast: contacts
#   pnpm ods:load:imessage   # contacts + iMessage
#   pnpm ods:load:full       # everything we can get
#   bash scripts/ods-load.sh --plan        # show the plan, run nothing
#
# Each source is its own canonical script (this wrapper adds no ingest logic):
#   scripts/contacts-sync.sh       Apple Contacts -> PROD (export/ODS/projection/mastery/names)
#   scripts/apple-sync.sh          iMessage -> PROD
#   scripts/apple-calls-sync.sh    Apple call history -> PROD
#   scripts/email-sync.sh          email metadata -> PROD
# Every one of them is idempotent/replay-safe, so re-running a flavor is harmless.
#
# NOT included: the calendar EventKit snapshot (app reads it directly, it is not a
# durable ingest) and anything DEV.
# ---------------------------------------------------------------------------
set -uo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

FLAVOR="contacts"
PLAN_ONLY=0
for arg in "$@"; do
  case "$arg" in
    contacts|imessage|full) FLAVOR="$arg" ;;
    --plan|--dry-run) PLAN_ONLY=1 ;;
    --help|-h) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "[ods-load] ERROR: unknown option '$arg' (try --help)" >&2; exit 2 ;;
  esac
done

log() { echo "[ods-load] $*"; }
fail() { echo "[ods-load] ERROR: $*" >&2; exit 1; }

# --- fail closed: this loader is PROD-only -----------------------------------
[ -f .env.local ] || fail "missing .env.local"
node --env-file=.env.local -e '
  const prod = process.env.DATABASE_URL_PROD
  const dev = process.env.DATABASE_URL_DEV
  if (!prod) { console.error("no DATABASE_URL_PROD configured"); process.exit(3) }
  if (dev && prod === dev) { console.error("DATABASE_URL_PROD equals DATABASE_URL_DEV"); process.exit(3) }
' || fail "PROD target check failed (refusing to load)"

# --- the flavor plan ---------------------------------------------------------
STEPS=()
case "$FLAVOR" in
  contacts) STEPS=("contacts") ;;
  imessage) STEPS=("contacts" "imessage") ;;
  full)     STEPS=("contacts" "imessage" "calls" "email") ;;
esac

script_for() {
  case "$1" in
    contacts) echo "$SELF_DIR/contacts-sync.sh" ;;
    imessage) echo "$SELF_DIR/apple-sync.sh" ;;
    calls)    echo "$SELF_DIR/apple-calls-sync.sh" ;;
    email)    echo "$SELF_DIR/email-sync.sh" ;;
  esac
}

log "flavor=$FLAVOR -> PROD (${STEPS[*]})"
if [ "$FLAVOR" = "contacts" ]; then
  log "iMessage is NOT part of this flavor (~90k messages, ~3h). Use: pnpm ods:load:imessage"
fi

if [ "$PLAN_ONLY" = "1" ]; then
  for s in "${STEPS[@]}"; do log "would run [$s] $(script_for "$s")"; done
  log "PLAN ONLY — nothing executed"
  exit 0
fi

RUN=()
for s in "${STEPS[@]}"; do
  script="$(script_for "$s")"
  [ -f "$script" ] || fail "$s: $script not found"
  log "--- $s ---"
  if ! bash "$script"; then
    fail "$s FAILED ($script). Later sources were NOT run. Every source is idempotent — fix and re-run the same flavor."
  fi
  RUN+=("$s")
done

echo
log "DONE — ran: ${RUN[*]}"
log "clients read models are refreshed by the contacts step"
