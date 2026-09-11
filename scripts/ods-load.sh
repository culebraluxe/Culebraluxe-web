#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — the ONE ODS loader. ALWAYS PROD. DEV is never a target.
#
# Flavors — every source can be loaded on its own:
#   contacts  (default, fast)  Apple Contacts, full field set
#                              export -> ODS staging -> l_person -> Person mastery
#                              -> names -> clients read models      (~1 minute)
#   imessage                   contacts + iMessage (~90k messages, ~3h)
#   gmail                      Gmail metadata ONLY (its own pull, nothing else)
#   calls                      contacts + Apple call history (incl. FaceTime)
#   full | all                 contacts + iMessage + calls + email
#
# Each communication source is a SEPARATE pull: run only the one you want.
# gmail is standalone — no contacts step, nothing implied. calls and imessage
# prepend the fast contacts step because their evidence stage needs contact
# identities resolved first; if you want contacts on its own, run ods:load.
#
#   pnpm ods:load            # fast: contacts
#   pnpm ods:load:imessage   # contacts + iMessage
#   pnpm ods:load:gmail      # contacts + Gmail
#   pnpm ods:load:calls      # contacts + calls/FaceTime
#   pnpm ods:load:all        # everything we can get
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
    contacts|imessage|email|gmail|calls|apple-mail|full|all) FLAVOR="$arg" ;;
    --plan|--dry-run) PLAN_ONLY=1 ;;
    --help|-h) sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "[ods-load] ERROR: unknown option '$arg' (try --help)" >&2; exit 2 ;;
  esac
done

# gmail is the captain's word for the email source; all is short for full.
[ "$FLAVOR" = "gmail" ] && FLAVOR="email"
[ "$FLAVOR" = "all" ] && FLAVOR="full"

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
  email)    STEPS=("email") ;;
  calls)    STEPS=("contacts" "calls") ;;
  apple-mail) STEPS=("contacts" "apple-mail") ;;
  full)     STEPS=("contacts" "imessage" "calls" "email") ;;
esac

script_for() {
  case "$1" in
    contacts) echo "$SELF_DIR/contacts-sync.sh" ;;
    imessage) echo "$SELF_DIR/apple-sync.sh" ;;
    calls)    echo "$SELF_DIR/apple-calls-sync.sh" ;;
    email)    echo "$SELF_DIR/email-sync.sh" ;;
    apple-mail) echo "$SELF_DIR/apple-mail-sync.sh" ;;
  esac
}

log "flavor=$FLAVOR -> PROD (${STEPS[*]})"
if [ "$FLAVOR" = "contacts" ]; then
  log "only contacts. Per-source loads: pnpm ods:load:imessage | ods:load:gmail | ods:load:calls | ods:load:all"
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
