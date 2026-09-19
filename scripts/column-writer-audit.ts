// ---------------------------------------------------------------------------
// ENG-FORGE-COLUMN-WRITER-01 — A field nothing writes is a lie.
//
// Every column of the three Forge run/evidence tables is classified as one of:
//
//   WRITTEN    — something durable writes it; the writer path is named and must
//                exist on disk.
//   DEAD-DROP  — nothing writes it and nothing reads it; it is a column that
//                describes a machine that does not exist.
//   DEAD-KEEP  — nothing writes it any more, but it is still READ; it stays, with
//                the reason it stays named.
//
// The audit is GENERATED, never hand-kept: `classify()` is pure and unit-tested,
// and `main()` reads the live schema read-only through the DatabaseGateway and
// refuses (non-zero exit) when a live column is unclassified or a declared column
// is gone. A declaration that names a writer which no longer exists is also a
// refusal — a map must not cite a dead path.
//
//   node --env-file=.env.local --import tsx scripts/column-writer-audit.ts          # write the manifest
//   node --env-file=.env.local --import tsx scripts/column-writer-audit.ts --check  # verify only, no write
// ---------------------------------------------------------------------------

import { existsSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { sql } from '../db/client'

export const AUDITED_TABLES = [
  'storyboard_story',
  'storyboard_story_run',
  'forge_workflow_evidence',
] as const

export type AuditedTable = (typeof AUDITED_TABLES)[number]

export type ColumnClassification =
  | { kind: 'WRITTEN'; writers: string[] }
  | { kind: 'DEAD-DROP'; reason: string }
  | { kind: 'DEAD-KEEP'; reason: string }

/** table -> column -> classification. The single authored source of truth. */
export type DeclaredColumns = Record<string, Record<string, ColumnClassification>>

/** table -> column names, as read from the schema. */
export type TableColumns = Record<string, string[]>

export type AuditClassification = 'WRITTEN' | 'DEAD-DROP' | 'DEAD-KEEP' | 'UNCLASSIFIED'

export type AuditRow = {
  table: string
  column: string
  classification: AuditClassification
  detail: string
}

export type AuditResult = {
  rows: AuditRow[]
  unclassified: Array<{ table: string; column: string }>
  /** Declared but absent from the schema — a stale claim, reported as a failure. */
  stale: Array<{ table: string; column: string }>
  /** WRITTEN entries whose writer path does not exist on disk. */
  missingWriters: Array<{ table: string; column: string; writer: string }>
}

const STORYBOARD = 'db/storyboard.ts'
const STORY_FILE = 'scripts/forge-test-stories.ts'
const SPRINT = 'db/sprint.ts'
/** 187: the sprint trigger derives storyboard_story.sprint_id from its batch. */
const SPRINT_TRIGGER = 'db/migrations/187_sprint_parent.sql'
const FORGE_RUN = 'db/forge-run.ts'
const REPAIR_LEDGER = 'db/forge-repair-ledger.ts'
const EVIDENCE = 'db/forge-workflow-evidence.ts'

/**
 * Every column of the three audited tables. `writers` names the module(s) that
 * write the column; `reason` on a DEAD-* entry is mandatory and non-empty.
 *
 * The one DEAD-DROP here is `lead_split_assignments`: added out-of-band by
 * migration 142, written by no module and read by no module. The DEAD-KEEP
 * columns are still read but have no live writer — the missing write path is the
 * defect the audit makes visible, not a reason to drop the column.
 */
export const DECLARED_COLUMNS: DeclaredColumns = {
  storyboard_story: {
    id: { kind: 'WRITTEN', writers: [STORYBOARD] },
    workstream: { kind: 'WRITTEN', writers: [STORYBOARD] },
    title: { kind: 'WRITTEN', writers: [STORYBOARD] },
    priority: { kind: 'WRITTEN', writers: [STORYBOARD] },
    status: { kind: 'WRITTEN', writers: [STORYBOARD] },
    notes: { kind: 'WRITTEN', writers: [STORYBOARD] },
    batch: { kind: 'WRITTEN', writers: [STORYBOARD] },
    goal: { kind: 'WRITTEN', writers: [STORYBOARD] },
    scope: { kind: 'WRITTEN', writers: [STORYBOARD] },
    acceptance_criteria: { kind: 'WRITTEN', writers: [STORYBOARD] },
    // 186: declared by the story author (or through the handoff) and normalized on write.
    acceptance_assertions: { kind: 'WRITTEN', writers: [STORYBOARD] },
    // 187: DERIVED from batch by the sprint trigger; db/sprint.ts writes both sides together on carry-over.
    sprint_id: { kind: 'WRITTEN', writers: [SPRINT_TRIGGER, SPRINT] },
    carried_over_from_sprint_id: { kind: 'WRITTEN', writers: [SPRINT] },
    dependencies: { kind: 'WRITTEN', writers: [STORYBOARD] },
    created_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    updated_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    completion: { kind: 'WRITTEN', writers: [STORYBOARD] },
    rollup: { kind: 'WRITTEN', writers: [STORYBOARD] },
    planned_start_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    actual_start_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    completed_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    preconditions: { kind: 'WRITTEN', writers: [STORYBOARD] },
    architect_brief: { kind: 'WRITTEN', writers: [STORYBOARD] },
    context_refs: { kind: 'WRITTEN', writers: [STORYBOARD] },
    postconditions: { kind: 'WRITTEN', writers: [STORYBOARD] },
    architect_brief_updated_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    operating_surface: { kind: 'WRITTEN', writers: [STORYBOARD] },
    test_mode: { kind: 'WRITTEN', writers: [STORYBOARD] },
    assay_commands: { kind: 'WRITTEN', writers: [STORYBOARD, STORY_FILE] },
    // Migration 196, declared the day it was added — because this audit was RED on DEV for exactly the
    // opposite reason on 2026-09-18 (five columns added by two stories and declared nowhere, found only
    // when its live half first ran). The control command is DECLARED here and WRITTEN by the filing path.
    negative_control_command: { kind: 'WRITTEN', writers: [STORYBOARD, STORY_FILE] },
    packet_sha: { kind: 'WRITTEN', writers: [STORYBOARD] },
    forge_repair_attempts: { kind: 'WRITTEN', writers: [REPAIR_LEDGER] },
    forge_replan_attempts: { kind: 'WRITTEN', writers: [REPAIR_LEDGER] },
    forge_last_qa_disposition: { kind: 'WRITTEN', writers: [REPAIR_LEDGER] },
    forge_last_failure_reason: { kind: 'WRITTEN', writers: [REPAIR_LEDGER] },
    forge_v1_legacy: {
      kind: 'DEAD-KEEP',
      reason:
        'No ongoing writer: backfilled once by migration 139 as a historical discriminator, and still READ by scripts/forge-consistency.ts. Kept because the legacy partition it names is real.',
    },
    batch_deploy: {
      kind: 'DEAD-KEEP',
      reason:
        'No durable writer exists — only a throwaway /tmp script and tests have ever set it. Still READ by workflow_app/forge/forge-facts.ts and scripts/forge-batch-release.mjs as the batch-release deferral flag, so it is kept and the missing write path is the defect.',
    },
  },
  storyboard_story_run: {
    id: { kind: 'WRITTEN', writers: [STORYBOARD] },
    story_id: { kind: 'WRITTEN', writers: [STORYBOARD] },
    started_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    ended_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    result_status: { kind: 'WRITTEN', writers: [STORYBOARD] },
    completion: { kind: 'WRITTEN', writers: [STORYBOARD] },
    notes: { kind: 'WRITTEN', writers: [STORYBOARD] },
    commit_hash: { kind: 'WRITTEN', writers: [STORYBOARD] },
    tests_summary: { kind: 'WRITTEN', writers: [STORYBOARD] },
    created_at: { kind: 'WRITTEN', writers: [STORYBOARD] },
    goal_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    preconditions_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    architect_brief_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    context_refs_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    acceptance_criteria_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    postconditions_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    updated_at: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    execution_environment: { kind: 'WRITTEN', writers: [STORYBOARD] },
    run_type: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    agent_runtime: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    scope_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    dependencies_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    operating_surface_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    test_mode_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    assay_commands_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    packet_sha_snapshot: { kind: 'WRITTEN', writers: [STORYBOARD, FORGE_RUN] },
    base_commit_hash: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    commands_total: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    commands_passed: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    commands_failed: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    tests_total: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    tests_passed: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    tests_failed: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    policy_violation_count: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    failure_code: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    evidence_detail: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    run_phase: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    lead_decision: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    lead_split_count: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    lead_split_assignments: {
      kind: 'DEAD-DROP',
      reason:
        'Nothing writes it and nothing reads it: added out-of-band by migration 142, superseded by lead_routing and lead_split_count on forge_workflow_evidence. A column that describes a machine that does not exist.',
    },
    model_used: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    tokens_input: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    tokens_output: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    cost_usd: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    cost_widgets: { kind: 'WRITTEN', writers: [STORYBOARD] },
    cost_source: { kind: 'WRITTEN', writers: [FORGE_RUN, STORYBOARD] },
    harness_session_id: { kind: 'WRITTEN', writers: [FORGE_RUN] },
    first_viol: { kind: 'WRITTEN', writers: [FORGE_RUN] },
  },
  forge_workflow_evidence: {
    process_instance_id: { kind: 'WRITTEN', writers: [EVIDENCE] },
    story_id: { kind: 'WRITTEN', writers: [EVIDENCE] },
    work_type: { kind: 'WRITTEN', writers: [EVIDENCE] },
    research_disposition: { kind: 'WRITTEN', writers: [EVIDENCE] },
    scout_required: { kind: 'WRITTEN', writers: [EVIDENCE] },
    root_cause_known: { kind: 'WRITTEN', writers: [EVIDENCE] },
    diagnosis_blocked: { kind: 'WRITTEN', writers: [EVIDENCE] },
    architecture_suspect: { kind: 'WRITTEN', writers: [EVIDENCE] },
    lead_decision: { kind: 'WRITTEN', writers: [EVIDENCE] },
    split_count: { kind: 'WRITTEN', writers: [EVIDENCE] },
    qa_review_required: { kind: 'WRITTEN', writers: [EVIDENCE] },
    qa_review_passed: { kind: 'WRITTEN', writers: [EVIDENCE] },
    qa_passed: { kind: 'WRITTEN', writers: [EVIDENCE] },
    failure_class: { kind: 'WRITTEN', writers: [EVIDENCE] },
    failed_release_stage: { kind: 'WRITTEN', writers: [EVIDENCE] },
    publish_succeeded: { kind: 'WRITTEN', writers: [EVIDENCE] },
    migration_required: { kind: 'WRITTEN', writers: [EVIDENCE] },
    migration_files: { kind: 'WRITTEN', writers: [EVIDENCE] },
    dev_migration_applied: { kind: 'WRITTEN', writers: [EVIDENCE] },
    dev_migration_verified: { kind: 'WRITTEN', writers: [EVIDENCE] },
    prod_migration_applied: { kind: 'WRITTEN', writers: [EVIDENCE] },
    prod_migration_verified: { kind: 'WRITTEN', writers: [EVIDENCE] },
    derived_refresh_required: { kind: 'WRITTEN', writers: [EVIDENCE] },
    derived_models: { kind: 'WRITTEN', writers: [EVIDENCE] },
    derived_refresh_succeeded: { kind: 'WRITTEN', writers: [EVIDENCE] },
    derived_refresh_verified: { kind: 'WRITTEN', writers: [EVIDENCE] },
    deployment_required: { kind: 'WRITTEN', writers: [EVIDENCE] },
    deployment_succeeded: { kind: 'WRITTEN', writers: [EVIDENCE] },
    deployment_receipt: { kind: 'WRITTEN', writers: [EVIDENCE] },
    production_verified: { kind: 'WRITTEN', writers: [EVIDENCE] },
    production_verification_receipt: { kind: 'WRITTEN', writers: [EVIDENCE] },
    resume_target: { kind: 'WRITTEN', writers: [EVIDENCE] },
    candidate_sha: { kind: 'WRITTEN', writers: [EVIDENCE] },
    qa_verified_sha: { kind: 'WRITTEN', writers: [EVIDENCE] },
    published_sha: { kind: 'WRITTEN', writers: [EVIDENCE] },
    deployed_sha: { kind: 'WRITTEN', writers: [EVIDENCE] },
    production_verified_sha: { kind: 'WRITTEN', writers: [EVIDENCE] },
    last_failure: { kind: 'WRITTEN', writers: [EVIDENCE] },
    // FOUND BY THIS AUDIT ON 2026-09-18, the first time its live half could actually run: five columns
    // exist in DEV because two stories added them and neither registered a writer here. The audit's own
    // contract is that a migration adding a column fails until the audit names its writer or its reason,
    // and it was RED for exactly that — unnoticed because the live half needs a database and nothing ran it.
    batch_released_sha: { kind: 'WRITTEN', writers: [EVIDENCE] },
    batch_released_at: { kind: 'WRITTEN', writers: [EVIDENCE] },
    batch_release_receipt: { kind: 'WRITTEN', writers: [EVIDENCE] },
    negative_control_ran: { kind: 'WRITTEN', writers: [EVIDENCE] },
    negative_control_killing_assertion: { kind: 'WRITTEN', writers: [EVIDENCE] },
    updated_at: { kind: 'WRITTEN', writers: [EVIDENCE] },
    created_at: { kind: 'WRITTEN', writers: [EVIDENCE] },
    findings: { kind: 'WRITTEN', writers: [EVIDENCE] },
    architecture_review_required: { kind: 'WRITTEN', writers: [EVIDENCE] },
    lead_routing: { kind: 'WRITTEN', writers: [EVIDENCE] },
    deployment_deferred_to_batch: { kind: 'WRITTEN', writers: [EVIDENCE] },
  },
}

/** The repository root, resolved from this file's own location (`<root>/scripts/`). */
export function repoRoot(): string {
  return resolve(dirname(fileURLToPath(import.meta.url)), '..')
}

function detailFor(classification: ColumnClassification): string {
  if (classification.kind === 'WRITTEN') return classification.writers.join(', ')
  return classification.reason
}

/**
 * PURE. Classify every column of the given schema against the declared map.
 * Reports the unclassified, the declared-but-gone, and the declared writers whose
 * path does not exist. No I/O, no database.
 */
export function classify(
  columns: TableColumns,
  declared: DeclaredColumns = DECLARED_COLUMNS,
  root: string = repoRoot(),
): AuditResult {
  const rows: AuditRow[] = []
  const unclassified: AuditResult['unclassified'] = []
  const missingWriters: AuditResult['missingWriters'] = []

  for (const table of Object.keys(columns).sort()) {
    for (const column of [...columns[table]].sort()) {
      const classification = declared[table]?.[column]
      if (!classification) {
        unclassified.push({ table, column })
        rows.push({
          table,
          column,
          classification: 'UNCLASSIFIED',
          detail: 'no declared writer and no declared reason',
        })
        continue
      }
      rows.push({
        table,
        column,
        classification: classification.kind,
        detail: detailFor(classification),
      })
      if (classification.kind === 'WRITTEN') {
        for (const writer of classification.writers) {
          if (!existsSync(resolve(root, writer))) {
            missingWriters.push({ table, column, writer })
          }
        }
      }
    }
  }

  const stale: AuditResult['stale'] = []
  for (const table of Object.keys(declared)) {
    const live = new Set(columns[table] ?? [])
    for (const column of Object.keys(declared[table])) {
      if (!live.has(column)) stale.push({ table, column })
    }
  }

  return { rows, unclassified, stale, missingWriters }
}

/** A generated, byte-stable render of the audit. No timestamp: regeneration is idempotent. */
export function renderManifest(result: AuditResult): string {
  const written = result.rows.filter((row) => row.classification === 'WRITTEN').length
  const deadDrop = result.rows.filter((row) => row.classification === 'DEAD-DROP').length
  const deadKeep = result.rows.filter((row) => row.classification === 'DEAD-KEEP').length
  const lines: string[] = []
  lines.push('# Column writer audit — run and evidence tables')
  lines.push('')
  lines.push(
    'GENERATED by `scripts/column-writer-audit.ts` — do not hand-edit. Every column of the three',
  )
  lines.push(
    'Forge run/evidence tables is classified WRITTEN (naming its writer), DEAD-DROP (nothing writes',
  )
  lines.push('it, nothing reads it) or DEAD-KEEP (nothing writes it, but something reads it).')
  lines.push('')
  lines.push('Regenerate and verify:')
  lines.push('')
  lines.push('```sh')
  lines.push('node --env-file=.env.local --import tsx scripts/column-writer-audit.ts')
  lines.push('node --env-file=.env.local --import tsx --test workflow_app/tests/db/column-writer-audit.test.ts')
  lines.push('```')
  lines.push('')
  lines.push(
    `Totals: ${result.rows.length} columns — ${written} WRITTEN, ${deadDrop} DEAD-DROP, ${deadKeep} DEAD-KEEP.`,
  )
  lines.push('')
  for (const table of AUDITED_TABLES) {
    const tableRows = result.rows.filter((row) => row.table === table)
    lines.push(`## ${table} (${tableRows.length})`)
    lines.push('')
    lines.push('| column | classification | writer / reason |')
    lines.push('| --- | --- | --- |')
    for (const row of tableRows) {
      lines.push(`| \`${row.column}\` | ${row.classification} | ${row.detail.replace(/\|/g, '\\|')} |`)
    }
    lines.push('')
  }
  if (result.stale.length > 0) {
    lines.push('## Stale declarations (declared, absent from the schema)')
    lines.push('')
    for (const row of result.stale) lines.push(`- \`${row.table}.${row.column}\``)
    lines.push('')
  }
  if (result.unclassified.length > 0) {
    lines.push('## UNCLASSIFIED (a live column with no declared writer or reason)')
    lines.push('')
    for (const row of result.unclassified) lines.push(`- \`${row.table}.${row.column}\``)
    lines.push('')
  }
  if (result.missingWriters.length > 0) {
    lines.push('## Declared writers that do not exist on disk')
    lines.push('')
    for (const row of result.missingWriters) {
      lines.push(`- \`${row.table}.${row.column}\` → \`${row.writer}\``)
    }
    lines.push('')
  }
  return `${lines.join('\n')}\n`
}

/** Read-only: the live column names of the audited tables, through the DatabaseGateway. */
export async function loadLiveColumns(): Promise<TableColumns> {
  const rows = (await sql`
    select table_name, column_name
    from information_schema.columns
    where table_schema = 'public'
      and table_name in ('storyboard_story', 'storyboard_story_run', 'forge_workflow_evidence')
    order by table_name, ordinal_position
  `) as Array<{ table_name: string; column_name: string }>
  const columns: TableColumns = {}
  for (const table of AUDITED_TABLES) columns[table] = []
  for (const row of rows) {
    if (!columns[row.table_name]) columns[row.table_name] = []
    columns[row.table_name].push(String(row.column_name))
  }
  return columns
}

// AN AUDIT IS NOT A MANIFEST, AND ITS FILE MUST NOT LIVE WHERE MANIFESTS DO. It was written to
// `docs/agent/manifest/COLUMN-WRITER-AUDIT.md` and the harness treats EVERY *.md in that directory as
// a manifest, rendering a fresh one for each and refusing any that differs — so this file could never
// satisfy it, and the release build failed for it (measured 2026-09-18: the release refused with
// "COLUMN-WRITER-AUDIT.md has drifted from a fresh render", 34 index rows to add). No release had run
// between this story shipping and that build, which is why the collision survived a whole batch.
const AUDIT_PATH = 'docs/agent/COLUMN-WRITER-AUDIT.md'

export async function main(argv: string[] = process.argv.slice(2)): Promise<number> {
  const checkOnly = argv.includes('--check')
  const columns = await loadLiveColumns()
  const result = classify(columns)
  const manifest = renderManifest(result)

  if (!checkOnly) {
    const { writeFileSync } = await import('node:fs')
    writeFileSync(resolve(repoRoot(), AUDIT_PATH), manifest, "utf8")
    console.log(`column-writer-audit: wrote ${AUDIT_PATH} (${result.rows.length} columns)`)
  } else {
    console.log(`column-writer-audit: checked ${result.rows.length} columns (no write)`)
  }

  const problems =
    result.unclassified.length + result.stale.length + result.missingWriters.length
  if (result.unclassified.length > 0) {
    console.error('column-writer-audit: UNCLASSIFIED columns — a column describes a machine nobody named:')
    for (const row of result.unclassified) console.error(`  - ${row.table}.${row.column}`)
  }
  if (result.stale.length > 0) {
    console.error('column-writer-audit: STALE declarations — declared column is gone from the schema:')
    for (const row of result.stale) console.error(`  - ${row.table}.${row.column}`)
  }
  if (result.missingWriters.length > 0) {
    console.error('column-writer-audit: DECLARED WRITER MISSING — the map cites a dead path:')
    for (const row of result.missingWriters) {
      console.error(`  - ${row.table}.${row.column} -> ${row.writer}`)
    }
  }
  return problems === 0 ? 0 : 1
}

const invokedDirectly = process.argv[1] ? /column-writer-audit\.(ts|js|mts)$/.test(process.argv[1]) : false
if (invokedDirectly) {
  main()
    .then((code) => {
      process.exitCode = code
    })
    .catch((error: unknown) => {
      console.error(`column-writer-audit: FAILED — ${error instanceof Error ? error.message : String(error)}`)
      process.exitCode = 1
    })
}
