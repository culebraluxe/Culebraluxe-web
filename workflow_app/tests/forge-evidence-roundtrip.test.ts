import { randomUUID } from 'node:crypto'
import assert from 'node:assert/strict'
import test from 'node:test'

import { sql } from '../../db/client'
import {
  mergeForgeWorkflowEvidence,
  readForgeWorkflowEvidence,
} from '../../db/forge-workflow-evidence'
import type { ForgeGateEvidence } from '../forge/forge-facts'

// ---------------------------------------------------------------------------
// ENG-FORGE serializer-assay: forge_workflow_evidence ROUND TRIP.
//
// DB-backed (DEV). Run on demand, NOT part of the DB-free test:app glob:
//
//   APP_ENV=development node --env-file=.env.local --import tsx \
//     --test-concurrency=1 --test workflow_app/tests/forge-evidence-roundtrip.test.ts
//
// Proves the canonical evidence writer (mergeForgeWorkflowEvidence) + reader
// (readForgeWorkflowEvidence) survive a real Postgres round trip with ACTUAL
// driver value shapes (integer split_count, jsonb findings, booleans, text
// arrays) — the normalization contract the repository owns. Cleans up after.
// ---------------------------------------------------------------------------

const DEV = 'development'

// DB-backed test: skip (not fail) when no database is configured, so the
// DB-free forge suite (pnpm test:forge:engine) stays green. It still runs on
// demand with the env file: APP_ENV=development node --env-file=.env.local ...
const dbConfigured = Boolean(
  process.env.DATABASE_URL_DEV || process.env.DATABASE_URL,
)

/** Minimal but valid parent rows the evidence FK requires. */
async function scaffold(story: string, processInstance: string): Promise<void> {
  const defs = await sql`select id from process_definitions where name = 'Forge Software Delivery Lifecycle' limit 1`
  const definitionId = String(defs[0]?.id ?? '')
  if (!definitionId) throw new Error('no Forge process_definition found in DEV')
  await sql`
    insert into storyboard_story (id, workstream, title, priority, status, notes, completion, rollup)
    values (${story}, 'Platform / Engineering / Data', 'forge evidence roundtrip fixture', 'High', 'Ready', 'temporary', 0, true)
    on conflict (id) do nothing
  `
  await sql`
    insert into process_instances (id, definition_id, subject_id, status, started_at, variables, version, created_at, updated_at)
    values (${processInstance}, ${definitionId}, ${story}, 'active', now(), '{}'::jsonb, 1, now(), now())
    on conflict (id) do nothing
  `
}

async function teardown(story: string, processInstance: string): Promise<void> {
  try {
    await sql`delete from forge_workflow_evidence where process_instance_id = ${processInstance}`
    await sql`delete from process_events where process_instance_id = ${processInstance}`
    await sql`delete from process_instances where id = ${processInstance}`
    await sql`delete from storyboard_story where id = ${story}`
  } catch {
    /* best-effort */
  }
}

test(
  'forge_workflow_evidence round-trips real driver shapes losslessly',
  { skip: dbConfigured ? false : 'DB not configured (run with .env.local on demand)' },
  async () => {
  process.env.APP_ENV = DEV
  const suffix = Date.now()
  const story = `EVIDENCE-RT-${suffix}`
  const pid = randomUUID()

  const full: ForgeGateEvidence = {
    workType: 'FEATURE',
    researchDisposition: 'IMPLEMENT',
    leadDecision: 'SPLIT',
    splitCount: 3,
    qaPassed: true,
    publishSucceeded: true,
    migrationRequired: true,
    migrationFiles: ['db/migrations/137_fixture.sql', 'db/migrations/138_fixture.sql'],
    derivedRefreshRequired: true,
    derivedModels: ['storyboard_story_run'],
    deploymentRequired: true,
    deploymentReceipt: 'deploy-fixture',
    findings: [
      {
        id: 'forge',
        summary: 'forge seam',
        required: true,
        seams: ['workflow_app/forge/'],
        hint: 'SAME_UNIT',
      },
      {
        id: 'adj',
        summary: 'adjacent TECH',
        required: false,
        seams: ['app/tech/'],
        hint: 'FOLLOW_UP_STORY',
      },
    ],
  }

  try {
    await scaffold(story, pid)
    // Write twice -> merge on conflict(process_instance_id) must NOT duplicate and
    // must preserve (coalesce) already-known truth across a second write.
    await mergeForgeWorkflowEvidence(pid, story, full)
    await mergeForgeWorkflowEvidence(pid, story, { workType: 'FEATURE' })

    const got = await readForgeWorkflowEvidence(story)

    assert.equal(got.workType, 'FEATURE')
    assert.equal(got.researchDisposition, 'IMPLEMENT')
    assert.equal(got.leadDecision, 'SPLIT')
    // Real integer column must come back as a JS number, not a string.
    assert.equal(got.splitCount, 3)
    assert.equal(typeof got.splitCount, 'number')
    assert.equal(got.qaPassed, true)
    assert.equal(got.migrationRequired, true)
    assert.deepEqual(got.migrationFiles, [
      'db/migrations/137_fixture.sql',
      'db/migrations/138_fixture.sql',
    ])
    assert.deepEqual(got.derivedModels, ['storyboard_story_run'])
    assert.equal(got.deploymentReceipt, 'deploy-fixture')
    // jsonb findings must parse to real objects with typed fields.
    assert.equal(got.findings?.length, 2)
    assert.equal(got.findings?.[0].id, 'forge')
    assert.equal(got.findings?.[0].required, true)
    assert.equal(got.findings?.[0].hint, 'SAME_UNIT')
    assert.deepEqual(got.findings?.[0].seams, ['workflow_app/forge/'])
    assert.equal(got.findings?.[1].hint, 'FOLLOW_UP_STORY')
    // Ledger observer counts normalize to numbers.
    assert.equal(typeof got.repairAttempts, 'number')
    assert.equal(typeof got.replanAttempts, 'number')
  } finally {
    await teardown(story, pid)
  }
})
