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
# Mail.app already holds the mail, so no API credentials are needed.
node -e '
const fs=require("fs");
const text=fs.readFileSync(".env.local","utf8");
const values=new Map(text.split(/\r?\n/).map(l=>l.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)).filter(Boolean).map(m=>[m[1],m[2].trim()]));
const missing=["DATABASE_URL_PROD"].filter(k=>!values.get(k));
if(missing.length){console.error("missing/empty env keys: "+missing.join(", "));process.exit(2)}
const accounts=values.get("MAIL_APP_ACCOUNTS")||values.get("APPLE_MAILBOX_ADDRESS")||values.get("ICLOUD_MAIL_ADDRESS");
if(!accounts){console.error("no Mail.app account configured (MAIL_APP_ACCOUNTS / APPLE_MAILBOX_ADDRESS / ICLOUD_MAIL_ADDRESS)");process.exit(2)}
' || fail "environment is incomplete"

# Local box -> bounded pages -> l_applemail. Full history by default; each page
# lands before the next is requested, and a crash resumes the same page.
log "PROD mailbox intake start (local Mail.app box -> L table)"
node --env-file=.env.local --import tsx scripts/apple-mailbox-intake.ts prod
log "PROD mailbox intake success"
log "sync complete"
