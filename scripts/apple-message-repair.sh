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
grep -qE '^DATABASE_URL_PROD=.+' .env.local || fail "DATABASE_URL_PROD missing/empty in .env.local"
[ -f "$EXPORT_DIR/manifest.json" ] || fail "manifest.json missing at $EXPORT_DIR"
[ -s "$EXPORT_DIR/identities.jsonl" ] || fail "identities.jsonl missing or empty"
[ -s "$EXPORT_DIR/messages.jsonl" ] || fail "messages.jsonl missing or empty"
command -v cargo >/dev/null 2>&1 || fail "cargo not found in PATH (the intake is Rust)"

message_count="$(wc -l < "$EXPORT_DIR/messages.jsonl" | tr -d ' ')"
handle_count="$(wc -l < "$EXPORT_DIR/identities.jsonl" | tr -d ' ')"
log "package OK: handles=$handle_count messages=$message_count"
log "rebuilding evidence only (no interaction replay), then refreshing the Client read models"

APP_ENV=production CULEBRALUXE_REPO="$REPO_ROOT" \
  cargo run -q --manifest-path "$REPO_ROOT/rust/Cargo.toml" -p cli -- \
  apple-sync messages-intake "$EXPORT_DIR" --evidence-only --refresh

log "PROD relationship evidence and all Client read models repaired"
