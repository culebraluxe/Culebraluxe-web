#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — canonical Gmail metadata -> PROD latest-context sync.
#
#   Gmail API (metadata only)  -> l_email                 (landing: source evidence)
#     -> cli gmail-sync                              (evidence -> interaction -> read models)
#
# Metadata only: no body, snippet, attachment or raw MIME is requested, transported or stored.
# Only exact-linked Gmail identities are read, each over a bounded window of its newest messages.
#
# Env (all required except the two optional ones):
#   GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET / GOOGLE_REFRESH_TOKEN   the gmail.readonly scope
#   CULEBRALUXE_GMAIL_WINDOW      messages per identity (default 25)
#   CULEBRALUXE_REPO              repository root (set by launchd / the deployed copy)
# ---------------------------------------------------------------------------
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="${CULEBRALUXE_REPO:-$(cd "$SELF_DIR/.." && pwd -P)}"
cd "$REPO_ROOT"

now() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log() { echo "[gmail-sync $(now)] $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

log "verifying environment"
[ -f .env.local ] || fail "missing .env.local"

# The Rust job fails closed on a missing credential and names the key; this check exists only to
# fail before cargo spends a minute compiling. It reads the same file the job does.
for key in DATABASE_URL_PROD GOOGLE_CLIENT_ID GOOGLE_CLIENT_SECRET GOOGLE_REFRESH_TOKEN; do
  grep -qE "^${key}=.+" .env.local || fail "$key is missing or empty in .env.local"
done

# APP_ENV=production is what makes the target PRODUCTION — the resolver refuses to guess, and the
# tally the job prints names the target it resolved.
log "PROD metadata intake start (Gmail metadata -> l_email -> interaction)"
APP_ENV=production CULEBRALUXE_REPO="$REPO_ROOT" \
  cargo run -q --manifest-path "$REPO_ROOT/Cargo.toml" -p cli -- \
  gmail-sync prod --window="${CULEBRALUXE_GMAIL_WINDOW:-25}" \
  || fail "Gmail metadata sync failed"

log "sync complete"
