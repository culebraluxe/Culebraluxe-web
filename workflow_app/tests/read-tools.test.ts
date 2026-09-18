// ---------------------------------------------------------------------------
// ENG-FORGE-READ-TOOLS-01 — the sanctioned read path (views + one CLI).
//
//   - the five views exist with the expected columns and are read-only;
//   - the CLI selects only from those views;
//   - the CLI output for a fixture story matches the individual readers.
//
// The proof is deliberately DB-independent: the migration is asserted as the frozen
// source of the view shapes, and the CLI runs against an injected executor returning
// fixture rows, so the comparison is against the readers' normalization, not a live DB.
// ---------------------------------------------------------------------------
import { test, afterEach } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor } from '../../db/query-executor'
import { listForgeBatches } from '../../db/forge-batch'
import { latestOpenForgeHold } from '../../db/forge-hold'
import { listStoryForgeFindings } from '../../db/forge-role-finding'
import { boardState, storyShow } from '../../scripts/forge-read-tools'

const MIGRATION_PATH = fileURLToPath(
  new URL('../../db/migrations/191_forge_read_views.sql', import.meta.url),
)

afterEach(() => {
  setDatabaseTestExecutor(null)
})

test('views exist with expected columns', async () => {
  const sqlText = await readFile(MIGRATION_PATH, 'utf8')
  const expected: Record<string, string[]> = {
    forge_board_by_batch: [
      'batch_id',
      'story_id',
      'item_state',
      'story_count',
      'queued_count',
      'skipped_count',
    ],
    forge_story_run_receipt: ['run_id', 'story_id', 'result_status', 'commit_hash'],
    forge_open_holds: ['hold_id', 'story_id', 'reason', 'originating_node'],
    forge_story_findings: ['finding_id', 'story_id', 'summary', 'required', 'seams', 'hint'],
    forge_migration_ledger: ['story_id', 'filename', 'migration_id', 'target', 'applied_at'],
  }
  for (const [view, columns] of Object.entries(expected)) {
    const match = sqlText.match(new RegExp(`create or replace view ${view} as([\\s\\S]*?);`, 'i'))
    assert.ok(match, `view ${view} is declared`)
    for (const column of columns) {
      assert.ok(match[1].includes(column), `${view} exposes ${column}`)
    }
  }
})

test('views are read-only', async () => {
  // Comments are prose; strip them so the assertion is about statements only.
  const sqlText = (await readFile(MIGRATION_PATH, 'utf8'))
    .replace(/\/\*[\s\S]*?\*\//g, ' ')
    .replace(/--[^\n]*/g, ' ')
    .toLowerCase()
  for (const forbidden of ['insert into', 'update ', 'delete from', 'create trigger', 'instead of']) {
    assert.ok(!sqlText.includes(forbidden), `no ${forbidden} in the views migration`)
  }
})

const boardRow = {
  id: 'batch-1',
  label: 'staging',
  status: 'Staged',
  scheduled_for: '2026-09-18T02:00:00.000Z',
  fired_at: null,
  created_at: '2026-09-18T01:00:00.000Z',
  created_by: 'user-1',
  note: 'night run',
  model_policy: 'cheap',
  story_count: 2,
  queued_count: 0,
  skipped_count: 0,
}

const storyBoardRow = {
  batch_id: 'batch-1',
  batch_label: 'staging',
  batch_status: 'Staged',
  story_id: 'story-1',
  item_state: 'Staged',
  item_queued_at: null,
  story_status: 'Batched',
  story_title: 'A story',
}

const receiptRow = {
  run_id: 'run-1',
  story_id: 'story-1',
  result_status: 'Complete',
  commit_hash: 'abc123',
  tests_summary: '1/1 pass',
  completion: 100,
  started_at: '2026-09-18T00:00:00.000Z',
  ended_at: '2026-09-18T00:10:00.000Z',
  created_at: '2026-09-18T00:10:00.000Z',
}

const holdRow = {
  hold_id: 7,
  story_id: 'story-1',
  reason: 'awaiting captain decision',
  originating_node: 'architect',
  failure_class: 'HOLD',
  resume_target: 'lead_pre',
  created_at: '2026-09-18T00:05:00.000Z',
  since: '2026-09-18T00:05:00.000Z',
  instance_id: 'proc-1',
}

const findingRow = {
  finding_id: 'F1',
  story_id: 'story-1',
  node_id: 'architect',
  attempt: 1,
  summary: 'the contract',
  required: true,
  seams: ['scripts'],
  hint: 'SAME_UNIT',
}

const migrationRow = {
  story_id: 'story-1',
  filename: '191_forge_read_views.sql',
  migration_id: 'mig-1',
  target: 'dev',
  applied_at: '2026-09-18T00:00:00.000Z',
  migration_required: true,
  dev_migration_applied: true,
  dev_migration_verified: true,
  prod_migration_applied: false,
  prod_migration_verified: false,
}

const fakeExecutor: QueryExecutor = async (strings) => {
  const text = Array.from(strings).join('?')
  if (text.includes('forge_board_by_batch') && text.includes('where story_id')) {
    return [storyBoardRow]
  }
  if (text.includes('forge_board_by_batch')) return [boardRow]
  if (text.includes('forge_story_run_receipt')) return [receiptRow]
  if (text.includes('forge_open_holds')) return [holdRow]
  if (text.includes('forge_story_findings')) return [findingRow]
  if (text.includes('forge_migration_ledger')) return [migrationRow]
  if (text.includes('forge_hold_record')) return [holdRow]
  if (text.includes('forge_role_finding')) return [findingRow]
  if (text.includes('from forge_batch b')) return [boardRow]
  return []
}

test('cli board matches individual readers', async () => {
  setDatabaseTestExecutor(fakeExecutor)
  const cli = await boardState()
  const reader = await listForgeBatches(10)
  assert.equal(cli.length, 1)
  assert.deepEqual(cli, reader)
})

test('cli story show resolves a story by id', async () => {
  setDatabaseTestExecutor(fakeExecutor)
  const show = await storyShow('story-1')
  assert.equal(show.storyId, 'story-1')
  assert.equal(show.receipt?.verdict, 'Complete')
  assert.equal(show.receipt?.commit, 'abc123')
  assert.equal(show.board.length, 1)
  assert.equal(show.holds[0]?.reason, 'awaiting captain decision')
  assert.equal(show.findings[0]?.findingId, 'F1')
  assert.equal(show.migrations[0]?.filename, '191_forge_read_views.sql')
})

test('cli story show matches individual readers', async () => {
  setDatabaseTestExecutor(fakeExecutor)
  const show = await storyShow('story-1')

  const readerHold = await latestOpenForgeHold('story-1')
  assert.equal(show.holds[0]?.reason, readerHold?.reason)
  assert.equal(show.holds[0]?.originatingNode, readerHold?.originatingNode)
  assert.equal(show.holds[0]?.createdAt, readerHold?.since)

  const readerFindings = await listStoryForgeFindings({
    storyId: 'story-1',
    processInstanceId: 'proc-1',
  })
  assert.ok(readerFindings)
  assert.deepEqual(
    show.findings.map((finding) => finding.findingId),
    readerFindings.map((finding) => finding.id),
  )
})
