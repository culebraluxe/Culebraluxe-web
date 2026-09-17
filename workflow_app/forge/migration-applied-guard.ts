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

import { readdirSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { join } from 'node:path'

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

// ---------------------------------------------------------------------------
// ENG-FORGE-MIGRATION-START-01 — no run STARTS while a migration in the repo is
// unapplied on PROD.
//
// The completion guard above sees a story that reaches the release lane. A story
// parked before DEV_OPS never gets there, and DEV_OPS owns migrations, so the same
// fact moves in FRONT of the run: read the repo file LIST and the LEDGER TABLE only
// (never a story diff — at start the work has not happened yet), and refuse to start
// when any db/migrations/*.sql is unledgered, naming every file. An unreadable ledger
// fails CLOSED to the named refusal, never to a silent start.
// ---------------------------------------------------------------------------

export type MigrationPreflightAssessment = {
  ok: boolean
  /** Every repo migration the ledger does not carry, in repo-list order. */
  unapplied: string[]
  /** The named refusal when ok is false, null otherwise. */
  refusal: string | null
}

/** The start-seam refusal, naming EVERY unapplied repo file (never a story diff). */
export function migrationPreflightRefusal(unapplied: readonly string[]): string {
  return (
    'repository migrations are not recorded in the PROD schema_migration ledger: ' +
    `${unapplied.join(', ')}`
  )
}

/**
 * The pure start decision: every db/migrations/*.sql file that CAN BE JUDGED must be carried by the
 * ledger. The repo list is a filesystem answer, not a change set — nothing has happened yet.
 *
 * THE BASELINE RULE (2026-09-17, same night, second pass): the ledger is authoritative only from its
 * `<baseline>` row forward, and the repo says so in its own words — `scripts/migration-status.mjs` calls
 * files absent from the ledger "unrecorded (pre-baseline or never applied here)" and refuses to claim
 * which. The first cut of this guard called every unledgered file "unapplied", so on this repo it saw
 * ~150 pre-baseline migrations and refused to START ANY RUN — a factory stop built by the guard meant to
 * protect it. `candidatePaths` is therefore the caller's answer to "which migrations can be judged",
 * which is the set ADDED SINCE THE BASELINE (git), and it defaults to the full repo list so the strict
 * behaviour still holds when the ledger carries no baseline row and claims full coverage.
 */
export function assessMigrationPreflight(input: {
  repoPaths: Iterable<string>
  ledgerFilenames: Iterable<string>
  /** Paths that postdate the ledger baseline. Absent = judge the whole repo list (no baseline row). */
  candidatePaths?: Iterable<string> | null
}): MigrationPreflightAssessment {
  const repoPaths = Array.from(input.repoPaths)
  const judgeable =
    input.candidatePaths == null ? repoPaths : repoPaths.filter((path) => new Set(input.candidatePaths).has(path))
  const assessment = assessMigrationApplied({
    changedPaths: judgeable,
    ledgerFilenames: input.ledgerFilenames,
  })
  return {
    ok: assessment.ok,
    unapplied: assessment.unapplied,
    refusal: assessment.ok ? null : migrationPreflightRefusal(assessment.unapplied),
  }
}

/** The repository-relative db/migrations/*.sql paths on disk, sorted. */
export function listRepoMigrationFiles(repoRoot: string = process.cwd()): string[] {
  const dir = join(repoRoot, 'db', 'migrations')
  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => `db/migrations/${entry.name}`)
    .sort()
}

/**
 * The migration files ADDED since `sinceIso` — the only ones that can be judged against the ledger when
 * a `<baseline>` row exists. One git call, read-only, and it answers a question the filesystem cannot:
 * which files are new enough to be this factory's responsibility.
 */
export function migrationsAddedSince(sinceIso: string, repoRoot: string = process.cwd()): string[] {
  const out = execFileSync(
    'git',
    ['log', '--diff-filter=A', '--name-only', '--format=', `--since=${sinceIso}`, '--', 'db/migrations'],
    { cwd: repoRoot, encoding: 'utf8' },
  )
  return Array.from(new Set(out.split('\n').map((line) => line.trim()).filter(Boolean))).sort()
}

/** The ledger's `<baseline>` timestamp for PROD, or null when the ledger claims coverage from the start. */
export async function readProdMigrationBaselineAt(): Promise<string | null> {
  const url = process.env.DATABASE_URL_PROD
  if (!url) {
    throw new Error('DATABASE_URL_PROD is not configured; the PROD migration baseline cannot be read')
  }
  const handle = forgeDb.forTarget(forgeDbTargetForUrl(url))
  const result = await handle.query<{ applied_at: Date | string }>(
    `select applied_at from schema_migration where target = 'prod' and filename = '<baseline>' order by applied_at desc limit 1`,
  )
  const first = result.rows[0]
  if (!first) return null
  return first.applied_at instanceof Date ? first.applied_at.toISOString() : String(first.applied_at)
}

/**
 * The start-seam preflight. Reads the repo file LIST and the PROD ledger, and NEVER throws:
 * an unreadable ledger fails CLOSED to the named refusal so the run refuses to start rather
 * than starting silently. The ledger reader is injectable for the both-directions proof.
 */
export async function preflightMigrationStart(
  input: {
    repoPaths?: Iterable<string>
    ledgerFilenames?: Iterable<string>
    readLedger?: () => Promise<string[]>
    /** The ledger baseline timestamp, injected for tests; null means the ledger claims full coverage. */
    baselineAt?: string | null
    readBaselineAt?: () => Promise<string | null>
    /** The post-baseline file set, injected for tests; only used when a baseline exists. */
    candidatePaths?: Iterable<string> | null
    readCandidatePaths?: (sinceIso: string) => string[]
  } = {},
): Promise<MigrationPreflightAssessment> {
  let repoPaths: string[]
  try {
    repoPaths = input.repoPaths ? Array.from(input.repoPaths) : listRepoMigrationFiles()
  } catch (error) {
    return {
      ok: false,
      unapplied: [],
      refusal: `could not read the repository migration list (db/migrations): ${String(
        (error as Error)?.message ?? error,
      )}; refusing to start`,
    }
  }
  let ledger: Iterable<string>
  let baselineAt: string | null
  try {
    ledger = input.ledgerFilenames ?? (await (input.readLedger ?? readProdMigrationLedger)())
    baselineAt =
      input.baselineAt !== undefined
        ? input.baselineAt
        : await (input.readBaselineAt ?? readProdMigrationBaselineAt)()
  } catch (error) {
    return {
      ok: false,
      unapplied: [],
      refusal: `could not read the PROD schema_migration ledger: ${String(
        (error as Error)?.message ?? error,
      )}; refusing to start`,
    }
  }
  // No baseline row: the ledger claims coverage from the start, so every repo file can be judged.
  if (baselineAt == null) {
    return assessMigrationPreflight({ repoPaths, ledgerFilenames: ledger })
  }
  // A baseline row means only migrations ADDED AFTER IT are this factory's to judge; everything older is
  // honestly "pre-baseline / unrecorded" and must not be reported as unapplied. A history that cannot be
  // read refuses by name rather than judging the whole repo and stopping the factory on pre-baseline files.
  let candidatePaths: Iterable<string>
  try {
    candidatePaths =
      input.candidatePaths !== undefined && input.candidatePaths !== null
        ? input.candidatePaths
        : (input.readCandidatePaths ?? ((since: string) => migrationsAddedSince(since)))(baselineAt)
  } catch (error) {
    return {
      ok: false,
      unapplied: [],
      refusal:
        `could not read the migration history since the ledger baseline (${baselineAt}): ${String(
          (error as Error)?.message ?? error,
        )}; refusing to start rather than judging pre-baseline migrations as unapplied`,
    }
  }
  return assessMigrationPreflight({ repoPaths, ledgerFilenames: ledger, candidatePaths })
}
