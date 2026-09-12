#!/usr/bin/env bash
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

now() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log() { echo "[email-sync $(now)] $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

log "verifying metadata-only authenticated Mail.app environment"
[ -f .env.local ] || fail "missing .env.local"
[ -d node_modules/tsx ] || fail "dependencies missing; run pnpm install"
command -v osascript >/dev/null 2>&1 || fail "osascript is required; run this sync on Lisa's Mac"

# Only the database target is required. The accounts to pull come from
# MAIL_APP_ACCOUNTS (falling back to APPLE_MAILBOX_ADDRESS / ICLOUD_MAIL_ADDRESS);
# Mail.app already holds the mail, so no API credentials are needed. The window comes
# from MAIL_SYNC_BAND and defaults to the newest band, 0-1, which is the last month.
node -e '
const fs=require("fs");
const text=fs.readFileSync(".env.local","utf8");
const values=new Map(text.split(/\r?\n/).map(l=>l.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)).filter(Boolean).map(m=>[m[1],m[2].trim()]));
const missing=["DATABASE_URL_PROD"].filter(k=>!values.get(k));
if(missing.length){console.error("missing/empty env keys: "+missing.join(", "));process.exit(2)}
const accounts=values.get("MAIL_APP_ACCOUNTS")||values.get("APPLE_MAILBOX_ADDRESS")||values.get("ICLOUD_MAIL_ADDRESS");
if(!accounts){console.error("no Mail.app account configured (MAIL_APP_ACCOUNTS / APPLE_MAILBOX_ADDRESS / ICLOUD_MAIL_ADDRESS)");process.exit(2)}
const band=(values.get("MAIL_SYNC_BAND")||"0-1").trim();
if(!["0-1","1-3","3-6","6-12","all"].includes(band)){console.error("MAIL_SYNC_BAND must be 0-1, 1-3, 3-6, 6-12 or all, got "+band);process.exit(2)}
process.stdout.write(band);
' > /tmp/culebraluxe-mail-band.$$ || fail "environment is incomplete"
BAND="$(cat /tmp/culebraluxe-mail-band.$$)"
rm -f /tmp/culebraluxe-mail-band.$$

# Local box -> bounded SQLite pages -> l_applemail, for every configured account.
# The newest band is re-read on every run, so a sync picks up what has arrived since;
# landing is replay-safe, so a re-read lands only genuinely new mail. If a run is ever
# suspect, wipe l_applemail and re-land rather than reconciling row by row.
log "PROD mailbox intake start (local Mail.app Envelope Index -> L table), band=$BAND"
node --env-file=.env.local --import tsx scripts/apple-mail-envelope-intake.ts prod --band="$BAND"
log "PROD mailbox intake success"

# PROMOTION — the one place an ODS (l_) table is read. Turns the landed source
# evidence into warehouse relationship memory: evidence reconciled to a canonical
# Person, then the canonical interaction the CRM pane reads, then a refresh of the
# client read models. Idempotent, so a re-run lands nothing twice.
log "PROD mail promotion start (l_applemail -> warehouse)"
node --env-file=.env.local --import tsx scripts/promote-applemail.ts prod --days="${MAIL_PROMOTE_DAYS:-90}"
log "PROD mail promotion success"
log "sync complete"
