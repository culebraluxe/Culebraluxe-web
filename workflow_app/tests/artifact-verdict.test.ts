import { test } from 'node:test'
import assert from 'node:assert/strict'

import { artifactVerdictForRun, recordToolArtifact } from '../../db/forge-artifact'
import type { QueryExecutor } from '../../db/query-executor'

// ---------------------------------------------------------------------------
// ENG-FORGE-ARTIFACT-RULING-01 — an artifact carries the ruling, never a second
// opinion. Every case runs through the write funnel with an in-memory executor;
// no DB/Neon required.
// ---------------------------------------------------------------------------

class FakeArtifactDb {
  /** The durable ruling the run already carries, or null when the run is unruled. */
  runRuling: string | null = 'Complete'
  runReadThrows = false
  inserted: { verdict: unknown; summary: unknown; kind: unknown } | null = null

  private norm(strings: TemplateStringsArray): string {
    return strings.join('?').replace(/\s+/g, ' ').trim().toLowerCase()
  }

  tx: QueryExecutor = (strings, ...params) => {
    const sql = this.norm(strings)
    if (sql.startsWith('select result_status from storyboard_story_run')) {
      if (this.runReadThrows) return Promise.reject(new Error('run read failed'))
      return Promise.resolve(this.runRuling === null ? [] : [{ result_status: this.runRuling }])
    }
    if (sql.startsWith('insert into forge_tool_artifact')) {
      const verdict = params[4] ?? null
      const summary = params[5] ?? null
      this.inserted = { verdict, summary, kind: params[3] ?? null }
      return Promise.resolve([
        {
          id: 'artifact-1',
          story_id: params[0],
          story_run_id: params[1],
          tool: params[2],
          kind: params[3],
          verdict,
          summary,
          sha: params[7] ?? null,
          created_at: new Date().toISOString(),
        },
      ])
    }
    return Promise.reject(new Error(`FAKE_ARTIFACT_DB_UNHANDLED: ${sql}`))
  }
}

test('run-verdict artifact agrees with its run\'s durable ruling', async () => {
  const db = new FakeArtifactDb()
  db.runRuling = 'Complete'
  const row = await recordToolArtifact(
    {
      storyId: 'story-1',
      storyRunId: 'run-1',
      tool: 'opencode',
      kind: 'run-verdict',
      verdict: 'Complete',
      summary: 'lane finished clean',
    },
    db.tx,
  )
  assert.equal(row.verdict, 'Complete')
  assert.equal(db.inserted?.verdict, 'Complete')
})

test('an artifact cannot be written with a verdict that contradicts its run', async () => {
  const db = new FakeArtifactDb()
  db.runRuling = 'Complete'
  const row = await recordToolArtifact(
    {
      storyId: 'story-1',
      storyRunId: 'run-1',
      tool: 'opencode',
      kind: 'run-verdict',
      verdict: 'Hold',
      summary: 'the lane said hold',
    },
    db.tx,
  )
  assert.equal(row.verdict, null)
  assert.equal(db.inserted?.verdict, null)
})

test('a contradicting artifact keeps its summary but carries no verdict', async () => {
  const db = new FakeArtifactDb()
  db.runRuling = 'Complete'
  const row = await recordToolArtifact(
    {
      storyId: 'story-1',
      storyRunId: 'run-1',
      tool: 'opencode',
      kind: 'run-verdict',
      verdict: 'Hold',
      summary: 'detail stays with the artifact',
      detail: { notes: 'a note that clarifies' },
    },
    db.tx,
  )
  assert.equal(row.verdict, null)
  assert.equal(row.summary, 'detail stays with the artifact')
})

test('a run-verdict artifact for an unruled run carries no verdict', async () => {
  const db = new FakeArtifactDb()
  db.runRuling = null
  const row = await recordToolArtifact(
    {
      storyId: 'story-1',
      storyRunId: 'run-1',
      tool: 'opencode',
      kind: 'run-verdict',
      verdict: 'Failed',
    },
    db.tx,
  )
  assert.equal(row.verdict, null)
  assert.equal(db.inserted?.verdict, null)
})

test('a failed ruling read fails closed to no verdict', async () => {
  const db = new FakeArtifactDb()
  db.runReadThrows = true
  const row = await recordToolArtifact(
    {
      storyId: 'story-1',
      storyRunId: 'run-1',
      tool: 'opencode',
      kind: 'run-verdict',
      verdict: 'Complete',
    },
    db.tx,
  )
  assert.equal(row.verdict, null)
})

test('an assay artifact keeps its own PASS verdict while the run is unruled', async () => {
  const db = new FakeArtifactDb()
  db.runRuling = null
  const row = await recordToolArtifact(
    {
      storyId: 'story-1',
      storyRunId: 'run-1',
      tool: 'assay',
      kind: 'qa-assay-evidence',
      verdict: 'PASS',
    },
    db.tx,
  )
  assert.equal(row.verdict, 'PASS')
})

test('the guard agrees by polarity, not by spelling', () => {
  assert.equal(artifactVerdictForRun({ kind: 'run-verdict', ruling: 'Complete', verdict: 'PASS' }), 'PASS')
  assert.equal(artifactVerdictForRun({ kind: 'run-verdict', ruling: 'Hold', verdict: 'Failed' }), 'Failed')
  assert.equal(artifactVerdictForRun({ kind: 'run-verdict', ruling: 'Complete', verdict: 'Hold' }), null)
  assert.equal(artifactVerdictForRun({ kind: 'run-verdict', ruling: null, verdict: 'Failed' }), null)
  assert.equal(artifactVerdictForRun({ kind: 'qa-assay-evidence', ruling: null, verdict: 'PASS' }), 'PASS')
})
