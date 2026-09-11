#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — canonical Apple Contacts -> PROD sync.
#
# One operator command for the complete Contacts lifecycle:
#   Apple Contacts (CNContactStore, local Mac)
#     -> contact-export/contacts-export.json
#     -> historical PROD ODS
#     -> landing tables (l_person, l_property)  <- current source state
#     -> THE promotion (promote-warehouse.ts)   <- the only reader of the landing tables
#     -> warehouse (person, property) + Clients materialized read models
#
# Nothing else reads the landing tables. Historical ODS is append/replay-safe.
# ---------------------------------------------------------------------------
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

EXPORT_DIR="$REPO_ROOT/contact-export"
EXPORT_FILE="$EXPORT_DIR/contacts-export.json"
TMP_EXPORT="$EXPORT_FILE.tmp.$$"

source "$SELF_DIR/contacts-sync-lock.sh"
LOCK_OWNED=0

cleanup() {
  rm -f "$TMP_EXPORT"
  if [ "$LOCK_OWNED" = "1" ]; then
    contacts_release_lock 2>/dev/null || true
  fi
}
trap cleanup EXIT

fail() {
  echo "[contacts-sync] ERROR: $*" >&2
  exit 1
}

log() {
  echo "[contacts-sync] $*"
}

[ -f .env.local ] || fail "missing .env.local at $REPO_ROOT"
[ -d "$EXPORT_DIR" ] || fail "missing contact-export package at $EXPORT_DIR"
command -v swift >/dev/null 2>&1 || fail "swift not found in PATH"
command -v node >/dev/null 2>&1 || fail "node not found in PATH"

if contacts_acquire_lock; then
  LOCK_OWNED=1
else
  fail "another Contacts sync is already running (lock=$CONTACTS_SYNC_LOCK pid=$(contacts_lock_pid))"
fi

node --env-file=.env.local -e '
if (!process.env.DATABASE_URL_PROD) process.exit(2)
if (process.env.DATABASE_URL_DEV && process.env.DATABASE_URL_PROD === process.env.DATABASE_URL_DEV) process.exit(3)
' || fail "DATABASE_URL_PROD missing or identical to DATABASE_URL_DEV"

SOURCE_ACCOUNT="${CULEBRALUXE_CONTACTS_SOURCE_ACCOUNT:-}"
if [ -z "$SOURCE_ACCOUNT" ]; then
  log "resolving existing PROD Apple Contacts source account"
  SOURCE_ACCOUNT="$(node --env-file=.env.local --input-type=module <<'NODE'
import { neon } from '@neondatabase/serverless'

const url = process.env.DATABASE_URL_PROD
if (!url) {
  console.error('DATABASE_URL_PROD is not configured')
  process.exit(2)
}
const sql = neon(url)
const rows = await sql`
  select distinct source_account
  from integration_intake_batch
  where source = 'apple_contacts'
    and source_account <> ''
  order by source_account
`
if (rows.length !== 1) {
  console.error(`Expected exactly one existing PROD apple_contacts source_account; found ${rows.length}. Set CULEBRALUXE_CONTACTS_SOURCE_ACCOUNT explicitly.`)
  process.exit(2)
}
process.stdout.write(String(rows[0].source_account))
NODE
  )" || fail "could not resolve a unique PROD Apple Contacts source account"
fi

[ -n "$SOURCE_ACCOUNT" ] || fail "Apple Contacts source account resolved empty"

log "exporting Apple Contacts from this Mac"
if ! swift run -c release --package-path "$EXPORT_DIR" contact-export > "$TMP_EXPORT"; then
  fail "Apple Contacts export failed; PROD ODS was not touched"
fi

log "validating fresh export"
node - "$TMP_EXPORT" <<'NODE' || exit 1
const fs = require('fs')
const file = process.argv[2]
let batch
try {
  batch = JSON.parse(fs.readFileSync(file, 'utf8'))
} catch (error) {
  console.error('[contacts-sync] ERROR: fresh Contacts export is not valid JSON')
  process.exit(2)
}
if (batch?.sourceSystem !== 'apple_contacts') {
  console.error('[contacts-sync] ERROR: fresh export sourceSystem is not apple_contacts')
  process.exit(2)
}
if (!Array.isArray(batch?.contacts) || batch.contacts.length === 0) {
  console.error('[contacts-sync] ERROR: fresh export contains no contacts')
  process.exit(2)
}
if (typeof batch?.exportId !== 'string' || !batch.exportId.trim()) {
  console.error('[contacts-sync] ERROR: fresh export has no exportId')
  process.exit(2)
}
console.log(`[contacts-sync] validated ${batch.contacts.length} contacts; exportId=${batch.exportId}`)
NODE

mv "$TMP_EXPORT" "$EXPORT_FILE"

# Notes come from the Contacts APP, not the Swift exporter: macOS gates CNContact.note
# behind the com.apple.developer.contacts.notes entitlement, which a local Swift build
# does not have (requesting the key silently yields ""). AppleScript reaches the app's
# own access instead. BULK property fetch (~5s); a per-person loop measured 336s.
# Non-fatal by design: notes are context, and a missing note must never block the load.
log "merging Apple Contacts NOTES into the fresh export"
if ! node --env-file=.env.local --import tsx scripts/merge-contacts-notes.ts --file "$EXPORT_FILE"; then
  log "WARNING: notes merge failed; continuing WITHOUT notes for this run"
fi

log "loading fresh export into historical PROD ODS"
if ! node --env-file=.env.local --import tsx scripts/load-apple-contacts.ts \
  --env prod \
  --file "$EXPORT_FILE" \
  --source-account "$SOURCE_ACCOUNT"; then
  fail "Contacts ODS load did not fully succeed (see loader output); no SUCCESS reported"
fi
log "ODS complete"

log "rebuilding current Contacts projection (l_person)"
if ! node --env-file=.env.local --import tsx scripts/project-apple-contacts.ts --env prod; then
  fail "Contacts current projection failed; no SUCCESS reported"
fi

log "promoting landing tables into the warehouse (person, property)"
if ! node --env-file=.env.local --import tsx scripts/promote-warehouse.ts --env prod --apply; then
  fail "landing -> warehouse promotion failed; Person/Property may be missing facts"
fi

log "SUCCESS: Apple Contacts -> ODS -> landing tables (l_person, l_property) -> warehouse (person, property) complete"
