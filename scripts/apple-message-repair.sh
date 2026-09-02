#!/usr/bin/env bash
# Repair Apple Messages relationship evidence and refresh PROD Client read models
# from the most recent validated local export. This intentionally skips the
# exporter and canonical interaction replay.
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
EXPORT_DIR="$REPO_ROOT/public/upload/data/apple-messages-export"

cd "$REPO_ROOT"

log() { printf '[apple-repair %s] %s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "$*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

log "verifying PROD environment and completed export package"
[ -f .env.local ] || fail "missing .env.local"
node -e '
const fs=require("fs");
const s=fs.readFileSync(".env.local","utf8");
for (const l of s.split(/\r?\n/)) {
  const t=l.trim();
  if (t.startsWith("DATABASE_URL_PROD=") && t.slice(18).trim()) process.exit(0);
}
process.exit(2);
' || fail "DATABASE_URL_PROD missing/empty in .env.local"
[ -f "$EXPORT_DIR/manifest.json" ] || fail "manifest.json missing at $EXPORT_DIR"
[ -s "$EXPORT_DIR/identities.jsonl" ] || fail "identities.jsonl missing or empty"
[ -s "$EXPORT_DIR/messages.jsonl" ] || fail "messages.jsonl missing or empty"
[ -d node_modules/tsx ] || fail "tsx not installed; run pnpm install"

message_count="$(wc -l < "$EXPORT_DIR/messages.jsonl" | tr -d ' ')"
handle_count="$(wc -l < "$EXPORT_DIR/identities.jsonl" | tr -d ' ')"
log "package OK: handles=$handle_count messages=$message_count"
log "rebuilding evidence only; progress prints every 100 identities"

node --env-file=.env.local --import tsx scripts/apple-messages-intake.ts \
  prod --dir "$EXPORT_DIR" --evidence-only

log "PROD relationship evidence and all Client read models repaired"
