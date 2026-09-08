#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { basename } from 'node:path'
import { upsertRegridCulebraParcels } from '../db/regrid-culebra-parcel'
import { parseRegridCulebraCsv } from '../services/regrid/csv'
import { createPoolExecutor } from './lib/pool-executor'

type CliArgs = {
  file: string
  env: 'dev' | 'prod'
  dryRun: boolean
}

function parseArgs(argv: string[]): CliArgs {
  let file: string | null = null
  let env: 'dev' | 'prod' = 'dev'
  let dryRun = false

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--env') {
      const value = argv[++i]
      if (value !== 'dev' && value !== 'prod') throw new Error('--env must be dev|prod')
      env = value
    } else if (arg === '--dry-run') {
      dryRun = true
    } else if (arg === '--') {
      continue
    } else if (!arg.startsWith('-') && !file) {
      file = arg
    }
  }

  if (!file) {
    throw new Error('usage: regrid-csv-load <regrid.csv> [--env dev|prod] [--dry-run]')
  }
  return { file, env, dryRun }
}

function printSummary(input: {
  file: string
  columns: number
  parsed: number
  accepted: number
  rejected: number
  withCatastro: number
  withPhysicalAddress: number
  withOwner: number
  withLatLon: number
}) {
  console.log([
    `File: ${basename(input.file)}`,
    `Columns: ${input.columns}`,
    `Rows parsed: ${input.parsed}`,
    `Accepted: ${input.accepted}`,
    `Rejected: ${input.rejected}`,
    `Catastro: ${input.withCatastro}`,
    `Physical address: ${input.withPhysicalAddress}`,
    `Owner: ${input.withOwner}`,
    `Lat/lon: ${input.withLatLon}`,
  ].join('\n'))
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const bytes = readFileSync(args.file)
  const content = bytes.toString('utf8')
  const sourceFileSha256 = createHash('sha256').update(bytes).digest('hex')
  const parsed = parseRegridCulebraCsv(content)

  printSummary({
    file: args.file,
    columns: parsed.headers.length,
    ...parsed.stats,
  })

  if (parsed.rejected.length > 0) {
    console.error('\nRejected rows:')
    for (const reject of parsed.rejected.slice(0, 10)) {
      console.error(`  row ${reject.sourceRowNumber}: ${reject.reason}`)
    }
    throw new Error('Refusing partial Regrid load while rejected rows exist')
  }
  if (args.dryRun) return

  const url = args.env === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  if (!url) throw new Error(`No DATABASE_URL_${args.env.toUpperCase()} configured (fail closed)`)
  if (args.env === 'prod' && url === process.env.DATABASE_URL_DEV) {
    throw new Error('PROD load selected but the configured connection is the DEV URL (fail closed)')
  }

  const { execute, end } = createPoolExecutor(url)
  try {
    const counts = await upsertRegridCulebraParcels(
      { sourceFileSha256, rows: parsed.rows },
      execute,
    )
    console.log(`\nDatabase target: ${args.env.toUpperCase()}`)
    console.log(`Changed: ${counts.changed}`)
    console.log(`Replay: ${counts.replayed}`)
  } finally {
    await end()
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
