#!/usr/bin/env node
// ---------------------------------------------------------------------------
// ENG-FORGE-READ-TOOLS-01 — the sanctioned read path for the daily read shapes.
//
//   forge:board            board counts by batch
//   forge:story:show <id>  a story's full picture: board, receipt, holds, findings, ledger
//
// The five read shapes live in read-only VIEWS (migration 191). This CLI SELECTS ONLY
// FROM THOSE VIEWS and never re-derives a fact from a base table, so a reader cannot
// invent a second interpretation of a fact.
//
// IDS AND TIMESTAMPS are normalized at this boundary to the driver form the existing
// readers already return: ids as strings, timestamps as ISO strings or null, and an
// unknown hold reason as "unknown" — never an empty string that reads like "no problem".
// ---------------------------------------------------------------------------
import { pathToFileURL } from 'node:url'

import { sql } from '@/legacy/db/client'
import { isoOrNull, mapForgeBatch, type ForgeBatch } from '@/legacy/db/forge-batch'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'

const str = (value: unknown): string | null => (value == null ? null : String(value))
const bool = (value: unknown): boolean | null => (value == null ? null : value === true)
const num = (value: unknown): number | null => {
  if (value == null) return null
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}
const strArray = (value: unknown): string[] => (Array.isArray(value) ? value.map(String) : [])

// ---------------------------------------------------------------------------
// Board by batch — the batch shape the existing reader (`listForgeBatches`) returns.
// ---------------------------------------------------------------------------
export async function boardState(execute: QueryExecutor = sql): Promise<ForgeBatch[]> {
  const rows = await execute`
    select
      batch_id           as id,
      batch_label        as label,
      batch_status       as status,
      batch_scheduled_for as scheduled_for,
      batch_fired_at     as fired_at,
      batch_created_at   as created_at,
      batch_created_by   as created_by,
      batch_note         as note,
      batch_model_policy as model_policy,
      story_count,
      queued_count,
      skipped_count
    from forge_board_by_batch
    order by batch_created_at desc, batch_id
  `
  const seen = new Set<string>()
  const batches: ForgeBatch[] = []
  for (const row of rows) {
    const id = String((row as QueryRow).id)
    if (seen.has(id)) continue
    seen.add(id)
    batches.push(mapForgeBatch(row as QueryRow))
  }
  return batches
}

export type StoryBoardRow = {
  batchId: string
  batchLabel: string | null
  batchStatus: string
  storyId: string
  itemState: string
  queuedAt: string | null
  storyStatus: string | null
  storyTitle: string | null
}

export async function storyBoard(
  storyId: string,
  execute: QueryExecutor = sql,
): Promise<StoryBoardRow[]> {
  const rows = await execute`
    select batch_id, batch_label, batch_status, story_id, item_state, item_queued_at,
           story_status, story_title
    from forge_board_by_batch
    where story_id = ${storyId}
    order by batch_created_at desc, batch_id
  `
  return rows.map((row) => ({
    batchId: String(row.batch_id),
    batchLabel: str(row.batch_label),
    batchStatus: String(row.batch_status),
    storyId: String(row.story_id),
    itemState: String(row.item_state),
    queuedAt: isoOrNull(row.item_queued_at),
    storyStatus: str(row.story_status),
    storyTitle: str(row.story_title),
  }))
}

// ---------------------------------------------------------------------------
// Latest run receipt — verdict and commit.
// ---------------------------------------------------------------------------
export type StoryReceipt = {
  runId: string
  storyId: string
  verdict: string | null
  commit: string | null
  testsSummary: string | null
  completion: number | null
  startedAt: string | null
  endedAt: string | null
  createdAt: string | null
}

export async function storyReceipt(
  storyId: string,
  execute: QueryExecutor = sql,
): Promise<StoryReceipt | null> {
  const rows = await execute`
    select run_id, story_id, result_status, commit_hash, tests_summary, completion,
           started_at, ended_at, created_at
    from forge_story_run_receipt
    where story_id = ${storyId}
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  return {
    runId: String(row.run_id),
    storyId: String(row.story_id),
    verdict: str(row.result_status),
    commit: str(row.commit_hash),
    testsSummary: str(row.tests_summary),
    completion: num(row.completion),
    startedAt: isoOrNull(row.started_at),
    endedAt: isoOrNull(row.ended_at),
    createdAt: isoOrNull(row.created_at),
  }
}

// ---------------------------------------------------------------------------
// Open holds — reason and node; blank reason is "unknown".
// ---------------------------------------------------------------------------
export type StoryHold = {
  holdId: number
  storyId: string
  reason: string
  originatingNode: string | null
  failureClass: string | null
  resumeTarget: string | null
  createdAt: string | null
}

export async function storyHolds(
  storyId: string,
  execute: QueryExecutor = sql,
): Promise<StoryHold[]> {
  const rows = await execute`
    select hold_id, story_id, reason, originating_node, failure_class, resume_target, created_at
    from forge_open_holds
    where story_id = ${storyId}
    order by created_at desc, hold_id desc
  `
  return rows.map((row) => {
    const reason = str(row.reason)
    return {
      holdId: num(row.hold_id) ?? 0,
      storyId: String(row.story_id),
      reason: reason && reason.trim() ? reason : 'unknown',
      originatingNode: str(row.originating_node),
      failureClass: str(row.failure_class),
      resumeTarget: str(row.resume_target),
      createdAt: isoOrNull(row.created_at),
    }
  })
}

// ---------------------------------------------------------------------------
// Story findings / contract.
// ---------------------------------------------------------------------------
export type StoryFinding = {
  findingId: string
  storyId: string
  nodeId: string
  attempt: number
  summary: string
  required: boolean
  seams: string[]
  hint: string | null
}

export async function storyFindings(
  storyId: string,
  execute: QueryExecutor = sql,
): Promise<StoryFinding[]> {
  const rows = await execute`
    select finding_id, story_id, node_id, attempt, summary, required, seams, hint
    from forge_story_findings
    where story_id = ${storyId}
    order by finding_id
  `
  return rows.map((row) => ({
    findingId: String(row.finding_id),
    storyId: String(row.story_id),
    nodeId: String(row.node_id),
    attempt: num(row.attempt) ?? 0,
    summary: String(row.summary),
    required: bool(row.required) ?? false,
    seams: strArray(row.seams),
    hint: str(row.hint),
  }))
}

// ---------------------------------------------------------------------------
// Migration ledger state for the story's change set.
// ---------------------------------------------------------------------------
export type StoryMigration = {
  filename: string
  migrationId: string | null
  target: string | null
  appliedAt: string | null
  migrationRequired: boolean | null
  devApplied: boolean | null
  devVerified: boolean | null
  prodApplied: boolean | null
  prodVerified: boolean | null
}

export async function storyMigrations(
  storyId: string,
  execute: QueryExecutor = sql,
): Promise<StoryMigration[]> {
  const rows = await execute`
    select story_id, filename, migration_id, target, applied_at, migration_required,
           dev_migration_applied, dev_migration_verified,
           prod_migration_applied, prod_migration_verified
    from forge_migration_ledger
    where story_id = ${storyId}
    order by filename
  `
  return rows.map((row) => ({
    filename: String(row.filename),
    migrationId: str(row.migration_id),
    target: str(row.target),
    appliedAt: isoOrNull(row.applied_at),
    migrationRequired: bool(row.migration_required),
    devApplied: bool(row.dev_migration_applied),
    devVerified: bool(row.dev_migration_verified),
    prodApplied: bool(row.prod_migration_applied),
    prodVerified: bool(row.prod_migration_verified),
  }))
}

// ---------------------------------------------------------------------------
// A story's full picture.
// ---------------------------------------------------------------------------
export type StoryShow = {
  storyId: string
  board: StoryBoardRow[]
  receipt: StoryReceipt | null
  holds: StoryHold[]
  findings: StoryFinding[]
  migrations: StoryMigration[]
}

export async function storyShow(
  storyId: string,
  execute: QueryExecutor = sql,
): Promise<StoryShow> {
  const [board, receipt, holds, findings, migrations] = await Promise.all([
    storyBoard(storyId, execute),
    storyReceipt(storyId, execute),
    storyHolds(storyId, execute),
    storyFindings(storyId, execute),
    storyMigrations(storyId, execute),
  ])
  return { storyId, board, receipt, holds, findings, migrations }
}

// ---------------------------------------------------------------------------
// CLI entry point.
// ---------------------------------------------------------------------------
async function main(): Promise<void> {
  const [, , command, ...rest] = process.argv
  if (command === 'board') {
    const batches = await boardState()
    process.stdout.write(`${JSON.stringify({ batches }, null, 2)}\n`)
    return
  }
  if (command === 'story:show') {
    const storyId = rest[0]
    if (!storyId) {
      process.stderr.write('usage: forge-read-tools story:show <story-id>\n')
      process.exit(2)
    }
    const show = await storyShow(storyId)
    process.stdout.write(`${JSON.stringify(show, null, 2)}\n`)
    return
  }
  process.stderr.write('usage: forge-read-tools <board|story:show> [story-id]\n')
  process.exit(2)
}

const invokedDirectly =
  process.argv[1] != null && import.meta.url === pathToFileURL(process.argv[1]).href

if (invokedDirectly) {
  main().catch((error) => {
    process.stderr.write(`forge-read-tools failed: ${String((error as Error)?.message ?? error)}\n`)
    process.exit(1)
  })
}
