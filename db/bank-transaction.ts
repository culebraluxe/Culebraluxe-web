import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import type { BankTransaction } from '../lib/bank-ofx'

// ---------------------------------------------------------------------------
// ACCOUNTING — OFX/QBO bank-transaction load repository (public.l_bank_transaction).
//
// Neutral load boundary for bank statement transactions. Follows the CulebraLuxe
// DB boundary convention: no React component touches SQL; reads/writes flow
// through the DatabaseGateway (QueryExecutor). Replay-safe: the unique partial
// index (source_system, source_account, fitid) is the final DB backstop, so
// loading the same file twice inserts zero duplicates and never touches the
// reconciliation state of existing rows.
//
// BANK EVIDENCE INTAKE ONLY — no expense creation, no reconciliation here.
// ---------------------------------------------------------------------------

export type BankImportCounts = {
  inserted: number
  replayed: number
  rejected: number
}

export type BankTransactionLoadInput = {
  sourceSystem: string
  sourceFormat: string
  sourceAccount: string
  bankId: string | null
  accountType: string | null
  currencyCode: string | null
  sourceFileSha256: string
  transactions: BankTransaction[]
}

/** Distinguish a row-level (constraint) failure from a system/connection failure. */
function isRowLevelError(e: unknown): boolean {
  const err = e as { code?: string; failure?: { code?: string }; message?: string }
  const code = err.code ?? err.failure?.code ?? ''
  if (code.startsWith('23')) return true
  return /constraint|violat/i.test(String(err.message ?? ''))
}

/**
 * Replay-safe insert of normalized bank transactions. Returns a deterministic
 * summary (inserted / replayed / rejected). A rejected transaction is counted
 * and skipped; a system/connection failure propagates (never masked as a
 * transaction rejection).
 */
export async function insertBankTransactions(
  input: BankTransactionLoadInput,
  execute: QueryExecutor = sql,
): Promise<BankImportCounts> {
  let inserted = 0
  let replayed = 0
  let rejected = 0

  for (const t of input.transactions) {
    // Defense-in-depth final validation gate (the parser already validates).
    if (!t.fitid || !t.postedAt || !Number.isFinite(t.amount)) {
      rejected++
      continue
    }
    try {
      const rows = (await execute`
        insert into l_bank_transaction (
          source_system, source_format, source_account, bank_id, account_type, currency_code,
          fitid, transaction_type, posted_at, user_initiated_at, amount, payee_name, memo,
          check_number, reference_number, source_file_sha256, raw_source_fragment
        ) values (
          ${input.sourceSystem}, ${input.sourceFormat}, ${input.sourceAccount}, ${input.bankId}, ${input.accountType}, ${input.currencyCode},
          ${t.fitid}, ${t.transactionType}, ${t.postedAt}, ${t.userInitiatedAt}, ${t.amount}, ${t.payeeName}, ${t.memo},
          ${t.checkNumber}, ${t.referenceNumber}, ${input.sourceFileSha256}, ${t.rawSourceFragment}
        )
        on conflict (source_system, source_account, fitid) do nothing
        returning id
      `) as { id: string }[]
      if (rows.length > 0) inserted++
      else replayed++
    } catch (e) {
      if (isRowLevelError(e)) {
        rejected++
        continue
      }
      throw e
    }
  }

  return { inserted, replayed, rejected }
}
