// ---------------------------------------------------------------------------
// ENG-FORGE-ARTIFACT-RESIDUE-01 — one-off, READ-ONLY residue sweep.
//
// NO TREES removed the per-lane worktrees; some stored records still name one.
// This sweep CLASSIFIES those rows as LEGACY with the story cutoff date and
// prints them. It writes nothing: the run policy forbids mutating production
// data, and a record that honestly says what it was is worth more than one that
// was quietly edited. Physically removing the historical values is a separate,
// authorized data change.
//
// Safe to run twice: classification is a pure function of the rows.
// ---------------------------------------------------------------------------

import { pathToFileURL } from 'node:url'

/** The date the no-tree-residue story landed. Rows written before it are LEGACY. */
export const STORY_CUTOFF_ISO = '2026-09-17'

/** Tokens that mark a record as naming a per-lane worktree or a tree-era field. */
export const RESIDUE_TOKENS = [
  'Culebraluxe-worktrees',
  '/worktrees/',
  'worktreePath',
  'worktree=',
] as const

/** Which of the residue tokens a stored text carries. */
export function residueTokensIn(text: string | null | undefined): string[] {
  const value = text ?? ''
  return RESIDUE_TOKENS.filter((token) => value.includes(token))
}

export type ResidueRow = {
  id: string
  source: string
  createdAt: string | null
  text: string
}

export type LegacyClassification = {
  id: string
  source: string
  createdAt: string | null
  residue: string[]
  /** True only for a pre-cutoff row that carries residue: LEGACY, not a defect. */
  legacy: boolean
  /** The date written beside the LEGACY marker; null when not legacy. */
  legacyAt: string | null
}

/**
 * Classify residue rows against the cutoff. A row written BEFORE the cutoff is
 * LEGACY and is stamped with the cutoff date. A row written at/after the cutoff
 * is a live defect (legacy=false) — the story's scan must fail on it. A row with
 * no residue is neither.
 */
export function classifyLegacyResidue(
  rows: readonly ResidueRow[],
  cutoffIso: string = STORY_CUTOFF_ISO,
): LegacyClassification[] {
  const cutoff = Date.parse(cutoffIso)
  return rows.map((row) => {
    const residue = residueTokensIn(row.text)
    const created = row.createdAt ? Date.parse(row.createdAt) : NaN
    const beforeCutoff = Number.isFinite(created) && created < cutoff
    const legacy = residue.length > 0 && beforeCutoff
    return {
      id: row.id,
      source: row.source,
      createdAt: row.createdAt,
      residue,
      legacy,
      legacyAt: legacy ? cutoffIso : null,
    }
  })
}

const RESIDUE_SQL_PATTERN = 'Culebraluxe-worktrees|/worktrees/|worktreePath|worktree='

async function main(): Promise<void> {
  const { sql } = await import('@/legacy/db/client')
  const rows: ResidueRow[] = []

  const artifacts = await sql`
    select id::text as id, created_at::text as created_at, detail::text as text
    from forge_tool_artifact
    where detail::text ~* ${RESIDUE_SQL_PATTERN}
  `
  for (const row of artifacts as ReadonlyArray<Record<string, unknown>>) {
    rows.push({
      id: String(row.id),
      source: 'forge_tool_artifact',
      createdAt: row.created_at == null ? null : String(row.created_at),
      text: String(row.text ?? ''),
    })
  }

  const runs = await sql`
    select id::text as id, created_at::text as created_at,
           coalesce(notes, '') || ' ' || coalesce(tests_summary, '') as text
    from storyboard_story_run
    where coalesce(notes, '') ~* ${RESIDUE_SQL_PATTERN}
       or coalesce(tests_summary, '') ~* ${RESIDUE_SQL_PATTERN}
  `
  for (const row of runs as ReadonlyArray<Record<string, unknown>>) {
    rows.push({
      id: String(row.id),
      source: 'storyboard_story_run',
      createdAt: row.created_at == null ? null : String(row.created_at),
      text: String(row.text ?? ''),
    })
  }

  const classified = classifyLegacyResidue(rows)
  const legacy = classified.filter((row) => row.legacy)
  const live = classified.filter((row) => !row.legacy)

  console.log(
    JSON.stringify(
      {
        cutoff: STORY_CUTOFF_ISO,
        readOnly: true,
        rowsScanned: rows.length,
        legacyCount: legacy.length,
        liveCount: live.length,
        legacy,
        live,
      },
      null,
      2,
    ),
  )
  console.log(
    `Tree-residue sweep: ${legacy.length} LEGACY row(s) marked ${STORY_CUTOFF_ISO}, ` +
      `${live.length} post-cutoff row(s) still carrying residue. No row was written.`,
  )
}

const isMain = process.argv[1]
  ? import.meta.url === pathToFileURL(process.argv[1]).href
  : false

if (isMain) {
  main().catch((error) => {
    console.error(`Tree-residue sweep failed: ${String((error as Error)?.message ?? error)}`)
    process.exitCode = 1
  })
}
