#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — ODS sync for everything EXCEPT the Apple Messages load.
#
# WHY THIS EXISTS: contacts and messages are SEPARATE pipelines. Running
# `pnpm apple:sync` (the command that looks like "the full upload") loads
# ~90k Messages and does NOT touch Contacts — which is how a contact entered
# in Apple Contacts on 2026-09-10 never appeared in "find client" while the
# newest contacts batch in PROD was 2026-09-07.
#
# Messages are the expensive source (~3 hours for ~90k messages) and are
# therefore NEVER part of a default run here. They are opt-in by name only.
#
#   bash scripts/ods-sync.sh                  # contacts only  (messages SKIPPED)
#   bash scripts/ods-sync.sh --with-calls     # + Apple Calls
#   bash scripts/ods-sync.sh --with-email     # + mail metadata (Gmail/iCloud)
#   bash scripts/ods-sync.sh --with-messages  # ALSO the ~3h Messages load — explicit only
#
# Env equivalent: ODS_SYNC_WITH="contacts,calls,email,messages"
#
# Each source is the canonical operator command for that source (this script
# adds no new ingest logic): scripts/contacts-sync.sh, scripts/apple-calls-sync.sh,
# scripts/email-sync.sh, scripts/apple-sync.sh.
# ---------------------------------------------------------------------------
set -uo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

WANT="${ODS_SYNC_WITH:-contacts}"
for arg in "$@"; do
  case "$arg" in
    --with-calls) WANT="$WANT,calls" ;;
    --with-email) WANT="$WANT,email" ;;
    --with-messages) WANT="$WANT,messages" ;;
    --help|-h)
      sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "[ods-sync] ERROR: unknown option $arg (try --help)" >&2; exit 2 ;;
  esac
done

has() { case ",$WANT," in *",$1,"*) return 0 ;; *) return 1 ;; esac; }
log() { echo "[ods-sync] $*"; }
fail() { echo "[ods-sync] ERROR: $*" >&2; exit 1; }

RUN=()
SKIPPED=()

run_source() {
  local name="$1" script="$2"
  [ -f "$script" ] || fail "$name: $script not found"
  log "running $name -> $script"
  if ! bash "$script"; then
    fail "$name FAILED ($script). Nothing further was run; re-run when fixed (each source is idempotent)."
  fi
  RUN+=("$name")
}

if has contacts; then run_source "contacts" "$SELF_DIR/contacts-sync.sh"; else SKIPPED+=("contacts"); fi
if has calls; then run_source "apple calls" "$SELF_DIR/apple-calls-sync.sh"; else SKIPPED+=("apple calls"); fi
if has email; then run_source "email metadata" "$SELF_DIR/email-sync.sh"; else SKIPPED+=("email metadata"); fi

if has messages; then
  log "running messages (this is the ~3h ~90k-message load; you asked for it)"
  if ! bash "$SELF_DIR/apple-sync.sh"; then fail "messages FAILED (scripts/apple-sync.sh)"; fi
  RUN+=("messages")
else
  SKIPPED+=("messages (~90k, ~3h — run 'pnpm apple:sync' only when you want it)")
fi

echo
log "DONE — ran: ${RUN[*]:-none}"
for s in "${SKIPPED[@]:-}"; do [ -n "$s" ] && log "skipped: $s"; done
log "Clients read models are refreshed by the contacts promotion step."
