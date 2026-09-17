// ---------------------------------------------------------------------------
// ENG-FORGE-ARTIFACT-RESIDUE-01 — no record carries a worktree path or a
// tree-era field.
//
// WHAT THIS SCAN SEES:
//   * the declared record-writer SOURCE files, checked for the tokens that name
//     a per-lane worktree or a tree-era field;
//   * the observer run-detail identity at runtime, checked for the removed field;
//   * when a database is configured, the stored rows written after the story
//     cutoff (forge_tool_artifact.detail, storyboard_story_run.notes/tests_summary).
//
// WHAT IT CANNOT SEE:
//   * a token assembled at runtime from concatenated string fragments;
//   * writers that were NOT declared as seams — notably the OpenCode harness
//     adapter, which builds its own `Run metadata: ... worktree=<cwd>` line from
//     the runtime workspace. That writer is a recorded follow-up, not silently
//     ignored; this comment is the honest boundary of the scan.
//
// The stored-record half of the acceptance is the companion SWEEP
// (scripts/forge-tree-residue-sweep.ts): it is a read-only generated scan over
// existing rows that marks pre-cutoff residue LEGACY and reports post-cutoff
// residue as a live defect. This test pins the sweep's classification contract
// (pre-cutoff => LEGACY+date, post-cutoff => defect, idempotent, writes nothing).
// ---------------------------------------------------------------------------

import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

import {
  STORY_CUTOFF_ISO,
  classifyLegacyResidue,
  type ResidueRow,
} from '../../scripts/forge-tree-residue-sweep'
import { createMemoryTraceSink } from '../forge/forge-observer/sink'
import { recordRunStart } from '../forge/forge-observer/record'

const HERE = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(HERE, '..', '..')

const PATH_TOKENS = ['Culebraluxe-worktrees', '/worktrees/', 'worktreePath'] as const
const NOTE_TOKENS = ['worktree='] as const

/**
 * A writer is scanned only for the tokens that would be residue IN THAT WRITER.
 * The harness adapters resolve the runtime workspace (executionWorkspace); only
 * their recorded note line is scanned here.
 *
 * The OPENCODE ADAPTER WAS MISSING FROM THIS LIST (2026-09-17) while it was the one
 * writer still emitting `worktree=`: the sweep's post-cutoff count caught it — 120 rows
 * written AFTER this story landed still carried the token — because the fence's claim
 * ("no record writer names a per-lane worktree") held only for the writers it scanned.
 * A fence is worth what its list covers.
 */
const RECORD_WRITERS: ReadonlyArray<{ file: string; tokens: readonly string[] }> = [
  { file: 'agent-runtime/candidate-assay-handoff.ts', tokens: [...PATH_TOKENS, ...NOTE_TOKENS] },
  { file: 'agent-runtime/agent-runtime-adapter.ts', tokens: [...PATH_TOKENS, ...NOTE_TOKENS] },
  {
    file: 'agent-runtime/deepseek/deepseek-harness-adapter.ts',
    tokens: ['Culebraluxe-worktrees', '/worktrees/', ...NOTE_TOKENS],
  },
  {
    file: 'agent-runtime/opencode/opencode-harness-adapter.ts',
    tokens: ['Culebraluxe-worktrees', '/worktrees/', ...NOTE_TOKENS],
  },
  {
    file: 'agent-runtime/deterministic-assay-adapter.ts',
    tokens: ['Culebraluxe-worktrees', '/worktrees/', ...NOTE_TOKENS],
  },
  { file: 'db/forge-artifact.ts', tokens: [...PATH_TOKENS, ...NOTE_TOKENS, 'workspace'] },
  { file: 'workflow_app/forge/forge-observer/types.ts', tokens: [...PATH_TOKENS, ...NOTE_TOKENS] },
  { file: 'workflow_app/forge/forge-observer/rehydrate.ts', tokens: [...PATH_TOKENS, ...NOTE_TOKENS] },
  {
    file: 'workflow_app/forge/agent-runtime-role-runner.ts',
    tokens: [...PATH_TOKENS, ...NOTE_TOKENS],
  },
]

test('no record writer names a per-lane worktree or a tree-era field', () => {
  for (const { file, tokens } of RECORD_WRITERS) {
    const source = readFileSync(resolve(REPO_ROOT, file), 'utf8')
    for (const token of tokens) {
      assert.ok(
        !source.includes(token),
        `${file} still contains the tree-era token ${JSON.stringify(token)}`,
      )
    }
  }
})

test('the observer run detail carries no worktree field at runtime', () => {
  const sink = createMemoryTraceSink()
  const event = recordRunStart(sink, {
    storyId: 'STORY-1',
    processInstanceId: 'proc-1',
    taskId: 'task-1',
    nodeId: 'smith',
    attempt: 1,
    baseCommit: 'a'.repeat(40),
  })
  assert.ok(!('worktreePath' in event), 'a run detail must not carry a worktree path')
  assert.equal(event.storyId, 'STORY-1')
  assert.equal(event.baseCommit, 'a'.repeat(40))
})

test('the sweep classifies pre-cutoff residue as LEGACY and is idempotent', () => {
  const rows: ResidueRow[] = [
    {
      id: 'old',
      source: 'forge_tool_artifact',
      createdAt: '2026-01-01T00:00:00Z',
      text: 'worktree=/worktrees/eng-qa-single-verdict-01',
    },
    {
      id: 'new',
      source: 'forge_tool_artifact',
      createdAt: '2026-09-18T00:00:00Z',
      text: 'worktree=/worktrees/eng-qa-single-verdict-01',
    },
    {
      id: 'clean',
      source: 'storyboard_story_run',
      createdAt: '2026-01-01T00:00:00Z',
      text: 'all good',
    },
  ]
  const first = classifyLegacyResidue(rows)
  const second = classifyLegacyResidue(rows)
  assert.deepEqual(first, second, 'classification is a pure function of the rows')
  const old = first.find((row) => row.id === 'old')
  const fresh = first.find((row) => row.id === 'new')
  assert.equal(old?.legacy, true)
  assert.equal(old?.legacyAt, STORY_CUTOFF_ISO)
  assert.equal(fresh?.legacy, false, 'post-cutoff residue is a defect, not LEGACY')
  assert.deepEqual(first.find((row) => row.id === 'clean')?.residue, [])
})

test('the sweep writes nothing', () => {
  const source = readFileSync(
    resolve(REPO_ROOT, 'scripts/forge-tree-residue-sweep.ts'),
    'utf8',
  ).toLowerCase()
  for (const write of ['insert into', 'update ', 'delete from']) {
    assert.ok(!source.includes(write), `the sweep must not run "${write.trim()}"`)
  }
})

test('the generated sweep scans both record stores and only reads', () => {
  const source = readFileSync(
    resolve(REPO_ROOT, 'scripts/forge-tree-residue-sweep.ts'),
    'utf8',
  ).toLowerCase()
  assert.ok(source.includes('from forge_tool_artifact'), 'the sweep must scan artifacts')
  assert.ok(source.includes('from storyboard_story_run'), 'the sweep must scan run notes')
  assert.ok(source.includes('select '), 'the sweep must be a generated read')
})
