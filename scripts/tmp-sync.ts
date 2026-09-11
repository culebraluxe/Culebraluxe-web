// TEMPORARY: reproduce the form's canonical sync and surface the real error.
// Run: APP_ENV=development node --conditions=react-server --env-file=.env.local --import tsx scripts/tmp-sync.ts <formId>
// TEMPORARY: reproduce the form's canonical sync and surface the real error.
// Run: APP_ENV=development node --conditions=react-server --env-file=.env.local --import tsx scripts/tmp-sync.ts <formId>
import { syncFormServiceBinding } from '../lib/forms/form-service-binding'

async function main() {
  const formId = process.argv[2]
  const actorId = process.argv[3] ?? null
  if (!formId) {
    console.error('usage: tmp-sync <formId> [actorId]')
    process.exit(2)
  }

  try {
    const result = await syncFormServiceBinding(formId, actorId)
    console.log('SYNC OK ->', JSON.stringify(result))
  } catch (error) {
    console.error('SYNC FAILED ->', error)
    if (error instanceof Error && error.stack) console.error(error.stack.split('\n').slice(0, 8).join('\n'))
  }
}

void main()

