#!/usr/bin/env bash
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

now() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log() { echo "[gmail-sync $(now)] $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

log "verifying environment"
[ -f .env.local ] || fail "missing .env.local"
[ -d node_modules/tsx ] || fail "dependencies missing; run pnpm install"

node -e '
const fs=require("fs");
const text=fs.readFileSync(".env.local","utf8");
const keys=new Set(text.split(/\r?\n/).map(l=>l.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)).filter(Boolean).filter(m=>m[2].trim()).map(m=>m[1]));
const required=["DATABASE_URL_DEV","DATABASE_URL_PROD","GOOGLE_CLIENT_ID","GOOGLE_CLIENT_SECRET","GOOGLE_REFRESH_TOKEN"];
const missing=required.filter(k=>!keys.has(k));
if(missing.length){console.error("missing/empty env keys: "+missing.join(", "));process.exit(2)}
' || fail "environment is incomplete"

log "DEV metadata intake start"
node --env-file=.env.local --import tsx scripts/gmail-metadata-sync.ts dev
log "DEV metadata intake success"

log "PROD metadata intake start"
node --env-file=.env.local --import tsx scripts/gmail-metadata-sync.ts prod
log "PROD metadata intake success"
log "sync complete"
