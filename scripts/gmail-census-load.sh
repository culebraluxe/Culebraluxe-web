#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — approved Gmail relationship census -> PROD relationship evidence.
#
#   approved census CSV (docs/marlowe-gmail-relationship-census-private-*.csv)
#     -> neutral evidence + the one adjudicator        (cli gmail-census)
#     -> integration_relationship_evidence             (the identities gmail-sync reads)
#
# This is the identity half of the Gmail chain, and the half that was missing: the metadata sync
# reads the exact-linked identities this load produces, so with no census load it has nothing to
# read. It was `scripts/rel-intel-load-gmail.ts` before the TypeScript engine was retired.
#
# The artifact is BOUNDED, operator-supplied and approval-gated: this is a load, not a pull. It never
# touches the Gmail API, and it must stay in the checkout — never under public/, which is the deploy
# artifact tree, and never somewhere it could be served.
#
# Replay-safe: evidence upserts on (source, source_account, source_identity_key) and an established
# canonical link survives a re-decision, so re-running the same artifact is harmless.
#
# Env:
#   CULEBRALUXE_CENSUS_FILE   artifact path (default: the approved batch in docs/)
#   CULEBRALUXE_REPO          repository root (set by launchd / the deployed copy)
# ---------------------------------------------------------------------------
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="${CULEBRALUXE_REPO:-$(cd "$SELF_DIR/.." && pwd -P)}"
cd "$REPO_ROOT"

now() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log() { echo "[gmail-census $(now)] $*"; }
fail() { log "ERROR: $*" >&2; exit 1; }

# One default, named here and in the CLI: the approved batch. A second artifact is chosen with
# CULEBRALUXE_CENSUS_FILE (or --file) deliberately, never guessed at by newest-file glob.
CENSUS_FILE="${CULEBRALUXE_CENSUS_FILE:-$REPO_ROOT/docs/marlowe-gmail-relationship-census-private-2026-08-24.csv}"

log "verifying environment"
[ -f .env.local ] || fail "missing .env.local"
[ -f "$CENSUS_FILE" ] || fail "census artifact not found: $CENSUS_FILE (set CULEBRALUXE_CENSUS_FILE to the approved batch)"

# APP_ENV=production is what makes the target PRODUCTION — the resolver refuses to guess, and the
# tally the job prints names the target it resolved.
log "PROD census load start (census CSV -> relationship evidence)"
APP_ENV=production CULEBRALUXE_REPO="$REPO_ROOT" \
  cargo run -q --manifest-path "$REPO_ROOT/Cargo.toml" -p cli -- \
  gmail-census prod --file="$CENSUS_FILE" \
  || fail "Gmail census load failed"

log "load complete — run scripts/gmail-sync.sh for latest context"