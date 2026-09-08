// Observability probe — proves the durable capture seam end-to-end against the
// LOCAL env target (DEV unless .env.local points at PROD).
//
// Writes ONE clearly-marked row to app_error via the real recordError path, then
// reads it back. Run:
//   node --env-file=.env.local --import tsx scripts/probe-error-capture.ts
//
// The printed MARKER can be pasted into the TECH view (or a SQL filter) to confirm
// the row landed. Uses level 'warn' so it doesn't pollute the error/fatal stream.
import '../db/client' // registers the gateway's sql executor for app_error
import { recordError, listRecentErrors } from '../db/app-error'

const MARKER = `PROBE-${Date.now()}`

async function main() {
  console.log('Writing probe row to app_error …')
  const written = await recordError({
    kind: 'PROBE',
    operation: 'probe:error-capture',
    code: 'PROBE_OK',
    message: `${MARKER} — error-capture seam probe. Delete me.`,
    retryable: false,
    route: '/probe/error-capture',
    level: 'warn',
    meta: { marker: MARKER },
  })
  console.log('written id:', written.id)
  console.log('MARKER:', MARKER)

  const recent = await listRecentErrors(50)
  const mine = recent.filter((r) => (r.message ?? '').includes(MARKER))
  console.log('rows matching marker:', mine.length)
  for (const row of mine) {
    console.log('---')
    console.log('  level     :', row.level)
    console.log('  kind      :', row.kind)
    console.log('  operation :', row.operation)
    console.log('  message   :', row.message)
    console.log('  created_at:', row.created_at)
  }
}

main().then(
  () => process.exit(0),
  (err) => {
    console.error('PROBE FAILED:', err)
    process.exit(1)
  },
)
