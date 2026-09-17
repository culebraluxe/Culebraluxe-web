// ---------------------------------------------------------------------------
// ENG-FORGE-MIGRATION-APPLIED-01 — a story that ships a migration is not complete
// while the control plane migration ledger lacks it.
//
// The migration ledger is `schema_migration` (migration 144), and a row's `filename`
// is the repository-relative path the migration command recorded — `db/migrations/<name>.sql`
// for target `prod`. This guard answers ONE question: does the PROD ledger carry every
// migration the story's change set adds? It reads the LEDGER TABLE only — never the
// filesystem (a file on disk is not proof it was applied) and never the DEV ledger.
//
// The assessment is pure and the ledger is injected, so the both-directions proof needs
// no database. Code must never land ahead of the column it writes.
// ---------------------------------------------------------------------------

import { forgeDb, forgeDbTargetForUrl } from '../../db/forge-db'

export type MigrationAppliedAssessment = {
  ok: boolean
  /** Every change-set migration the ledger does not carry, in change-set order. */
  unapplied: string[]
}

/** The canonical ledger filename for a repository-relative path, or null when it is not a migration. */
export function migrationLedgerFilename(path: string): string | null {
  const normalized = path.trim().replace(/\\/g, '/').replace(/^\.\//, '')
  return /^db\/migrations\/[^/]+\.sql$/i.test(normalized) ? normalized : null
}

/** Every db/migrations/*.sql path a change set adds, de-duplicated, in change-set order. */
export function migrationFilesInChangeSet(changedPaths: readonly string[]): string[] {
  const seen = new Set<string>()
  const files: string[] = []
  for (const path of changedPaths) {
    const file = migrationLedgerFilename(path)
    if (file && !seen.has(file)) {
      seen.add(file)
      files.push(file)
    }
  }
  return files
}

/**
 * The pure decision: every migration the change set adds must be carried by the ledger.
 * A change set with no db/migrations/*.sql path is unaffected (ok, no unapplied names).
 */
export function assessMigrationApplied(input: {
  changedPaths: readonly string[]
  ledgerFilenames: Iterable<string>
}): MigrationAppliedAssessment {
  const files = migrationFilesInChangeSet(input.changedPaths)
  if (files.length === 0) return { ok: true, unapplied: [] }
  const ledger = new Set<string>()
  for (const filename of input.ledgerFilenames) {
    const normalized = migrationLedgerFilename(filename)
    if (normalized) ledger.add(normalized)
  }
  const unapplied = files.filter((file) => !ledger.has(file))
  return { ok: unapplied.length === 0, unapplied }
}

/** The refusal, naming EVERY unapplied file. */
export function migrationAppliedRefusal(unapplied: readonly string[]): string {
  return (
    'story change set adds migration(s) not recorded in the PROD schema_migration ledger: ' +
    `${unapplied.join(', ')}`
  )
}

/**
 * Read the PROD ledger filenames. Reads the LEDGER table only, and refuses when the PROD
 * connection is not configured rather than falling back to DEV.
 *
 * The shared pool is deliberately NOT ended: it is process-global, and a read must not
 * tear down a pool another caller is still using.
 */
export async function readProdMigrationLedger(): Promise<string[]> {
  const url = process.env.DATABASE_URL_PROD
  if (!url) {
    throw new Error('DATABASE_URL_PROD is not configured; the PROD migration ledger cannot be read')
  }
  const handle = forgeDb.forTarget(forgeDbTargetForUrl(url))
  const result = await handle.query<{ filename: string }>(
    `select filename from schema_migration where target = 'prod'`,
  )
  return result.rows.map((row) => String(row.filename))
}

/** Assess a change set against the PROD ledger (or an injected ledger). */
export async function guardMigrationApplied(input: {
  changedPaths: readonly string[]
  ledgerFilenames?: Iterable<string>
}): Promise<MigrationAppliedAssessment> {
  const ledger = input.ledgerFilenames ?? (await readProdMigrationLedger())
  return assessMigrationApplied({ changedPaths: input.changedPaths, ledgerFilenames: ledger })
}
