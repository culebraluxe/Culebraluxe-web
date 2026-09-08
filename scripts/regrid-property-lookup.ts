// Regrid one-parcel prototype.
//
// Address-only lookup (read-only, one HTTP request):
//   REGRID_API_TOKEN=... pnpm exec tsx scripts/regrid-property-lookup.ts \
//     "123 Example St, Culebra, PR 00775"
//
// Enrich one existing Property (one HTTP request + one canonical Property update):
//   REGRID_API_TOKEN=... pnpm exec tsx scripts/regrid-property-lookup.ts \
//     --property <property-uuid>
//
// Optional narrower Regrid path:
//   --path /us/pr/<county-slug>
// or REGRID_PATH=...

import '../db/client'
import { recordError } from '../db/app-error'
import { SqlRegridPropertyEnrichmentRepository } from '../db/regrid-property-enrichment'
import {
  enrichPropertyFromRegrid,
  regridClientFromEnv,
} from '../services/regrid'

function parseArgs(argv: string[]) {
  let propertyId: string | null = null
  let path: string | null | undefined = undefined
  const address: string[] = []

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--property') {
      propertyId = argv[++i]?.trim() || null
      if (!propertyId) throw new Error('--property requires a Property UUID')
      continue
    }
    if (arg === '--path') {
      path = argv[++i]?.trim() || null
      if (!path) throw new Error('--path requires a Regrid path')
      continue
    }
    address.push(arg)
  }

  return {
    propertyId,
    path,
    address: address.join(' ').trim() || null,
  }
}

function print(value: unknown) {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`)
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const client = regridClientFromEnv()

  if (args.propertyId) {
    const repository = new SqlRegridPropertyEnrichmentRepository()
    const result = await enrichPropertyFromRegrid({
      propertyId: args.propertyId,
      client,
      repository,
      path: args.path,
      queryOverride: args.address,
    })
    print(result)
    if (result.status === 'ambiguous') process.exitCode = 2
    if (result.status === 'not_found') process.exitCode = 3
    return
  }

  if (!args.address) {
    throw new Error(
      'Pass a street address, or use --property <uuid> to enrich an existing Property.',
    )
  }

  const result = await client.lookupAddress({
    query: args.address,
    path: args.path,
    limit: 2,
  })
  print(result)
  if (result.status === 'ambiguous') process.exitCode = 2
  if (result.status === 'not_found') process.exitCode = 3
}

main().catch(async (error) => {
  const message = error instanceof Error ? error.message : String(error)
  console.error('REGRID LOOKUP FAILED:', message)

  // The CLI is an outer operation boundary, so failures are captured through
  // the canonical app_error seam. A logger failure is printed but never hides
  // the original Regrid/property failure.
  try {
    await recordError({
      kind: 'EXTERNAL_INTEGRATION',
      operation: 'regrid:property-lookup',
      code: 'REGRID_LOOKUP_FAILED',
      message,
      retryable: true,
      level: 'error',
      meta: {
        integration: 'regrid',
        mode: process.argv.includes('--property') ? 'property_enrichment' : 'address_lookup',
      },
    })
  } catch (captureFailure) {
    console.error(
      'Could not persist app_error for Regrid failure:',
      captureFailure instanceof Error ? captureFailure.message : String(captureFailure),
    )
  }
  process.exitCode = 1
})
