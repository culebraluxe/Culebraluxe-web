#!/usr/bin/env node
// ---------------------------------------------------------------------------
// BANK-OFX — command-line QBO/OFX bank transaction loader (dry-run + load).
//
// Usage:
//   pnpm bank:load:dev -- /path/to/file.qbo
//   pnpm bank:load:prod -- /path/to/file.qbo
//   pnpm bank:load:dev -- /path/to/file.qbo --dry-run
//
// Parses a QBO/OFX file, then either prints an import summary (--dry-run) or
// loads the normalized transactions replay-safely into public.l_bank_transaction.
// BANK EVIDENCE INTAKE ONLY — no expense creation, no reconciliation.
//
// Never prints full account numbers, connection credentials, or entire raw files.
// ---------------------------------------------------------------------------
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { basename } from 'node:path'
import {
  formatMoney,
  maskAccount,
  parseBankStatement,
  sourceAccountIdentity,
} from '../lib/bank-ofx'
import { insertBankTransactions } from '../db/bank-transaction'
import { createPoolExecutor } from './lib/pool-executor'

const SOURCE_SYSTEM = 'ofx_qbo'

type CliArgs = {
  file: string
  env: 'dev' | 'prod'
  dryRun: boolean
}

function parseArgs(argv: string[]): CliArgs {
  let file: string | undefined
  let env: 'dev' | 'prod' = 'dev'
  let dryRun = false
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--env') {
      const v = argv[i + 1] ?? 'dev'
      if (v !== 'dev' && v !== 'prod') throw new Error('--env must be dev|prod')
      env = v
      i++
    } else if (a === '--dry-run') {
      dryRun = true
    } else if (a === '--') {
      // pnpm arg separator; the file path follows.
      if (!file && argv[i + 1]) file = argv[i + 1]
      i++
    } else if (!file && !a.startsWith('-')) {
      file = a
    }
  }
  if (!file) throw new Error('usage: bank-transaction-load <file.qbo|file.ofx> [--env dev|prod] [--dry-run]')
  return { file, env, dryRun }
}

function sha256(content: string): string {
  return createHash('sha256').update(content).digest('hex')
}

function dateOnly(iso: string | null): string | null {
  return iso ? iso.slice(0, 10) : null
}

function printSummary(opts: {
  file: string
  format: string
  account: string | null
  bankId: string | null
  currency: string | null
  periodStart: string | null
  periodEnd: string | null
  parsed: number
  valid: number
  inserted: number
  replayed: number
  rejected: number
  debits: number
  credits: number
}) {
  const lines = [
    `File: ${basename(opts.file)}`,
    `Format: ${opts.format}`,
    `Account: ${opts.account ?? '(none)'}`,
    `Bank ID: ${opts.bankId ?? '(none)'}`,
    `Currency: ${opts.currency ?? '(none)'}`,
    `Period: ${dateOnly(opts.periodStart) ?? '?'} → ${dateOnly(opts.periodEnd) ?? '?'}`,
    '',
    `Parsed:    ${opts.parsed}`,
    `Valid:     ${opts.valid}`,
    `Inserted:  ${opts.inserted}`,
    `Replay:    ${opts.replayed}`,
    `Rejected:  ${opts.rejected}`,
    '',
    `Debits:   ${formatMoney(opts.debits)}`,
    `Credits:  ${formatMoney(opts.credits)}`,
  ]
  console.log(lines.join('\n'))
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const content = readFileSync(args.file, 'utf8')

  const parsed = parseBankStatement(content, basename(args.file))
  const s = parsed.statement
  const sourceAccount = sourceAccountIdentity(s.accountId)

  const debits = s.transactions.reduce((sum, t) => sum + Math.min(0, t.amount), 0)
  const credits = s.transactions.reduce((sum, t) => sum + Math.max(0, t.amount), 0)

  const formatLabel = `${s.sourceFormat} / ${s.syntaxKind === 'xml' ? 'XML OFX' : 'legacy OFX'}`

  if (args.dryRun) {
    printSummary({
      file: args.file,
      format: formatLabel,
      account: maskAccount(s.accountId),
      bankId: s.bankId,
      currency: s.currencyCode,
      periodStart: s.statementStart,
      periodEnd: s.statementEnd,
      parsed: parsed.parsedCount,
      valid: s.transactions.length,
      inserted: 0,
      replayed: 0,
      rejected: parsed.rejected.length,
      debits,
      credits,
    })
    return
  }

  // Resolve the target database (fail closed; never generic DATABASE_URL).
  const url =
    args.env === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  if (!url) {
    throw new Error(`No DATABASE_URL_${args.env.toUpperCase()} configured (fail closed)`)
  }
  if (args.env === 'prod' && url === process.env.DATABASE_URL_DEV) {
    throw new Error('PROD load selected but the configured connection is the DEV URL (fail closed)')
  }

  const { execute, end } = createPoolExecutor(url)
  try {
    const counts = await insertBankTransactions(
      {
        sourceSystem: SOURCE_SYSTEM,
        sourceFormat: s.sourceFormat,
        sourceAccount,
        bankId: s.bankId,
        accountType: s.accountType,
        currencyCode: s.currencyCode,
        sourceFileSha256: sha256(content),
        transactions: s.transactions,
      },
      execute,
    )
    printSummary({
      file: args.file,
      format: formatLabel,
      account: maskAccount(s.accountId),
      bankId: s.bankId,
      currency: s.currencyCode,
      periodStart: s.statementStart,
      periodEnd: s.statementEnd,
      parsed: parsed.parsedCount,
      valid: s.transactions.length,
      inserted: counts.inserted,
      replayed: counts.replayed,
      rejected: parsed.rejected.length + counts.rejected,
      debits,
      credits,
    })
  } finally {
    await end()
  }
}

main().catch((err) => {
  console.error((err as Error)?.message ?? String(err))
  process.exit(1)
})
