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

node -e '
const fs=require("fs");
const text=fs.readFileSync(".env.local","utf8");
const values=new Map(text.split(/\r?\n/).map(l=>l.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)).filter(Boolean).map(m=>[m[1],m[2].trim()]));
const required=["DATABASE_URL_DEV","DATABASE_URL_PROD","ICLOUD_MAIL_ADDRESS","EMAIL_INTERNAL_ADDRESSES"];
const missing=required.filter(k=>!values.get(k));
if(missing.length){console.error("missing/empty env keys: "+missing.join(", "));process.exit(2)}
' || fail "environment is incomplete"

log "DEV Apple Mail metadata sync start"
node --env-file=.env.local --import tsx scripts/icloud-mail-sync.ts dev
log "DEV Apple Mail metadata sync success"

log "PROD Apple Mail metadata sync start"
node --env-file=.env.local --import tsx scripts/icloud-mail-sync.ts prod
log "PROD Apple Mail metadata sync success"
log "sync complete"
