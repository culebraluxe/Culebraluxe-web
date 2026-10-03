#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — canonical Apple Mail -> PROD sync.
#
#   local Mail.app Envelope Index (read-only, via the Python bridge)
#     -> l_applemail                          (landing: source evidence, no judgment)
#     -> cli apple-sync mail-promote     (evidence -> reconcile -> interaction)
#     -> Client read models
#
# Metadata only: no bodies, snippets, attachments or raw MIME are read, shipped or stored.
# PROD-only by design, and the run is refused if DATABASE_URL_PROD equals DATABASE_URL_DEV.
#
# Env (all optional except the database target):
#   MAIL_APP_ACCOUNTS         comma-separated Mail.app accounts
#                             (fallback APPLE_MAILBOX_ADDRESS / ICLOUD_MAIL_ADDRESS)
#   MAIL_SYNC_BAND            0-1 | 1-3 | 3-6 | 6-12 | all   (default 0-1)
#   MAIL_PROMOTE_DAYS         promotion window in days       (default 90)
#   EMAIL_INTERNAL_ADDRESSES  required by the promotion: it decides inbound vs outbound
#   CULEBRALUXE_REPO          repository root (set by launchd / the deployed copy)
# ---------------------------------------------------------------------------
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="${CULEBRALUXE_REPO:-$(cd "$SELF_DIR/.." && pwd -P)}"
cd "$REPO_ROOT"

now() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log() { echo "[email-sync $(now)] $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

log "verifying metadata-only authenticated Mail.app environment"
[ -f .env.local ] || fail "missing .env.local"
command -v osascript >/dev/null 2>&1 || fail "osascript is required; run this sync on Lisa's Mac"
command -v python3 >/dev/null 2>&1 || fail "python3 is required to read Mail's Envelope Index"

BAND="${MAIL_SYNC_BAND:-0-1}"
case "$BAND" in
  0-1|1-3|3-6|6-12|all) ;;
  *) fail "MAIL_SYNC_BAND must be 0-1, 1-3, 3-6, 6-12 or all, got $BAND" ;;
esac

# The accounts, the band and the window are configuration; the Rust job resolves them itself and
# refuses when a required one is missing. APP_ENV=production is what makes the target PRODUCTION —
# the resolver refuses to guess, and the tally it prints names the target it resolved.
run_cli() {
  APP_ENV=production CULEBRALUXE_REPO="$REPO_ROOT" \
    cargo run -q --manifest-path "$REPO_ROOT/Cargo.toml" -p cli -- "$@"
}

# Local box -> bounded SQLite pages -> l_applemail, for every configured account. The newest band
# is re-read on every run, so a sync picks up what has arrived since; landing is replay-safe, so a
# re-read lands only genuinely new mail. If a run is ever suspect, wipe l_applemail and re-land
# rather than reconciling row by row.
log "PROD mailbox intake start (local Mail.app Envelope Index -> l_applemail), band=$BAND"
run_cli apple-sync mail-intake prod --band="$BAND" \
  || fail "mail intake failed; nothing was promoted"

# PROMOTION — the one place a mail landing table is read. Turns landed source evidence into
# warehouse relationship memory: evidence reconciled to a canonical Person, then the canonical
# interaction the CRM pane reads, then a refresh of the client read models. Idempotent, so a
# re-run lands nothing twice.
log "PROD mail promotion start (l_applemail -> warehouse)"
run_cli apple-sync mail-promote prod --days="${MAIL_PROMOTE_DAYS:-90}" \
  || fail "mail promotion failed"

log "sync complete"
