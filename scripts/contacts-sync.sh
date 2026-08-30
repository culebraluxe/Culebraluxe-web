#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — canonical Apple Contacts -> PROD ODS sync.
#
# One operator command for the complete Contacts intake cycle:
#   Apple Contacts (CNContactStore, local Mac)
#     -> contact-export/contacts-export.json (private, gitignored)
#     -> scripts/load-apple-contacts.ts --env prod
#     -> PROD ODS integration_intake_batch / integration_inbox /
#        integration_staged_contact_profile
#
# This script deliberately stops at ODS staging. It does NOT run the SUPPORT-2
# l_person projection and does NOT mutate canonical person/person_identity.
#
# Source-account identity is never guessed. By default the script resolves the
# one existing PROD apple_contacts source_account. An explicit override may be
# supplied with CULEBRALUXE_CONTACTS_SOURCE_ACCOUNT.
# ---------------------------------------------------------------------------
set -euo pipefail

SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)"
cd "$REPO_ROOT"

EXPORT_DIR="$REPO_ROOT/contact-export"
EXPORT_FILE="$EXPORT_DIR/contacts-export.json"
TMP_EXPORT="$EXPORT_FILE.tmp.$$"

cleanup() {
  rm -f "$TMP_EXPORT"
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

# Fail closed unless PROD is explicitly configured.
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

log "loading fresh export into PROD ODS"
node --env-file=.env.local --import tsx scripts/load-apple-contacts.ts \
  --env prod \
  --file "$EXPORT_FILE" \
  --source-account "$SOURCE_ACCOUNT"

log "SUCCESS: Apple Contacts export -> PROD ODS intake complete"
