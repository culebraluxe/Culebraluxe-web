// ---------------------------------------------------------------------------
// ENG-08 — Story Execution Evidence — targeted unit tests.
//
// Proves the concise tests/checks summary extraction for the DeepSeek harness
// evidence path deterministically, zero Neon, zero React:
//   - extractTestsSummary keeps the LAST deliberate `Tests: ...` line in the
//     assistant's final report and returns its trimmed content
//   - absent / malformed markers fall back to the caller's factual fallback
//     (the harness exit code) — the tests summary is never fabricated from
//     free-form prose
//   - overlong summaries are truncated to the concise bound
//   - buildTaskText asks the model to END its report with the `Tests: <summary>`
//     line, without leaking vendor nouns or the test-mode directive token
// The resultExternal SCOPED-violation override is covered by the adjacent
// real-Postgres adapter suite (agent-runtime-deepseek.test.ts).
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  TESTS_SUMMARY_MAX_LENGTH,
  TESTS_SUMMARY_MARKER,
  buildTaskText,
  extractTestsSummary,
  workspaceEvidenceLine,
} from '../../agent-runtime/deepseek/deepseek-harness-adapter'
import type { AgentWorkCommand } from '../../agent-runtime/types'
import type { StoryboardStory } from '../../db/storyboard'

function command(overrides: Partial<AgentWorkCommand> = {}): AgentWorkCommand {
  return {
    workItemId: 'wi-evidence-1',
    storyId: 'ENG-08',
    role: 'builder',
    modelProfile: 'builder-flash',
    specialInstructions: null,
    priority: 50,
    state: 'Ready',
    claimedBy: null,
    claimedAt: null,
    startedAt: null,
    finishedAt: null,
    storyRunId: null,
    errorText: null,
    runtimeAdapter: null,
    externalRunId: null,
    attempts: 0,
    maxAttempts: 3,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    ...overrides,
  }
}

function story(overrides: Partial<StoryboardStory> = {}): StoryboardStory {
  return {
    id: 'ENG-08',
    workstream: 'HARDEN',
    operatingSurface: null,
    title: 'Story Execution Evidence',
    priority: 'Medium',
    status: 'Planned',
    notes: 'Record commit, tests and concise execution result against a human-authored story.',
    batch: null,
    goal: 'Record commit, tests and concise execution result against a human-authored story.',
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    architectBriefUpdatedAt: null,
    completion: 0,
    rollup: true,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
    createdAt: '2026-08-21T00:00:00.000Z',
    updatedAt: '2026-08-21T00:00:00.000Z',
    ...overrides,
  }
}

const FALLBACK = 'dsh exit code 0'

test('ENG-08: extractTestsSummary returns the last deliberate Tests line', () => {
  const output = [
    'Implemented the evidence summary extraction.',
    'Earlier prose mentions "Tests:" inside a sentence are not the evidence line.',
    'Verification (SCOPED policy): new workflow_app/tests/evidence-summary.test.ts 8/8 pass; tsc --noEmit clean.',
    'Tests: workflow_app/tests/evidence-summary.test.ts 8/8 pass; tsc --noEmit clean; next build passed',
  ].join('\n')
  assert.equal(
    extractTestsSummary(output, FALLBACK),
    'workflow_app/tests/evidence-summary.test.ts 8/8 pass; tsc --noEmit clean; next build passed',
  )
})

test('ENG-08: extractTestsSummary keeps the LAST line when multiple markers exist', () => {
  const output = [
    'Tests: first run 1/1',
    'Tests: final run 2/2 pass',
  ].join('\n')
  assert.equal(extractTestsSummary(output, FALLBACK), 'final run 2/2 pass')
})

test('ENG-08: extractTestsSummary tolerates leading bullets and whitespace', () => {
  assert.equal(
    extractTestsSummary('  - Tests:    evidence-summary 8/8 pass  ', FALLBACK),
    'evidence-summary 8/8 pass',
  )
})

test('ENG-08: extractTestsSummary does not treat lowercase or embedded prose as the marker', () => {
  const output = [
    'tests: lowercase is not the marker',
    'I ran the tests: but this is prose',
    'AllTests: not a boundary match',
  ].join('\n')
  assert.equal(extractTestsSummary(output, FALLBACK), FALLBACK)
})

test('ENG-08: extractTestsSummary falls back on null/empty/marker-less output', () => {
  assert.equal(extractTestsSummary(null, FALLBACK), FALLBACK)
  assert.equal(extractTestsSummary(undefined, FALLBACK), FALLBACK)
  assert.equal(extractTestsSummary('', FALLBACK), FALLBACK)
  assert.equal(extractTestsSummary('no marker here', FALLBACK), FALLBACK)
  assert.equal(extractTestsSummary('Tests:', FALLBACK), FALLBACK, 'empty value is not a summary')
  assert.equal(extractTestsSummary('Tests:   ', FALLBACK), FALLBACK, 'whitespace-only value is not a summary')
})

test('ENG-08: extractTestsSummary truncates overlong summaries to the concise bound', () => {
  const long = 'x'.repeat(TESTS_SUMMARY_MAX_LENGTH + 200)
  const extracted = extractTestsSummary(`Tests: ${long}`, FALLBACK)
  assert.ok(extracted.length <= TESTS_SUMMARY_MAX_LENGTH)
  assert.ok(extracted.endsWith('…'), 'truncation is explicit')
  assert.ok(extracted.startsWith('x'.repeat(TESTS_SUMMARY_MAX_LENGTH - 1)))
})

test('ENG-08: buildTaskText instructs the model to end with a Tests summary line', () => {
  const text = buildTaskText(command(), {
    command: command(),
    story: story(),
    policy: { allowCommit: true, allowDevDbWrite: true, allowControlPlaneWrite: true },
    capabilities: [],
    executionEnvironment: 'DEV',
    storyRunId: 'run-1',
  })
  assert.match(text, /End your final report with one concise "Tests: <summary>" line/)
  assert.ok(text.includes(TESTS_SUMMARY_MARKER))
  assert.match(text, /so the harness can record a concrete tests\/checks summary against this story/)
  // The evidence instruction never leaks vendor nouns or the test-mode directive.
  assert.doesNotMatch(text, /deepseek|DeepSeek|v4|session/i)
  assert.doesNotMatch(text, /\[runtime\s+test-mode:/i)
})

test('ENG-08: buildTaskText keeps the SCOPED policy authoritative above the evidence line', () => {
  const text = buildTaskText(
    command({ specialInstructions: 'story prose asks for pnpm test' }),
    {
      command: command({ specialInstructions: 'story prose asks for pnpm test' }),
      story: story(),
      policy: { allowCommit: true, allowDevDbWrite: true, allowControlPlaneWrite: true },
      capabilities: [],
      executionEnvironment: 'DEV',
      storyRunId: 'run-1',
    },
  )
  assert.match(text, /FULL regression is NOT authorized/)
  assert.match(text, /Tests: <summary>/)
})

// ---------------------------------------------------------------------------
// ENG-21 — isolated worker workspace evidence.
// ---------------------------------------------------------------------------

test('ENG-21: workspaceEvidenceLine identifies branch, worktree and base commit', () => {
  const line = workspaceEvidenceLine({
    branchName: 'agent/eng-21/9f3c2b1a',
    worktreePath: '/worktrees/eng-21-9f3c2b1a',
    baseRef: 'main',
    baseCommit: 'eeb68e2'.padEnd(40, '0'),
  })
  assert.equal(
    line,
    'Execution workspace: branch=agent/eng-21/9f3c2b1a worktree=/worktrees/eng-21-9f3c2b1a base=main@' +
      'eeb68e2'.padEnd(40, '0'),
  )
})

test('ENG-21: buildTaskText tells the model its isolated branch/base only when a workspace was provisioned', () => {
  const base = {
    command: command(),
    story: story(),
    policy: { allowCommit: true, allowDevDbWrite: true, allowControlPlaneWrite: true },
    capabilities: [],
    executionEnvironment: 'DEV',
    storyRunId: 'run-1',
  }
  const baseCommit = 'eeb68e2'.padEnd(40, '0')

  const isolated = buildTaskText(command(), {
    ...base,
    executionWorkspace: {
      branchName: 'agent/eng-21/9f3c2b1a',
      worktreePath: '/worktrees/eng-21-9f3c2b1a',
      baseRef: 'main',
      baseCommit,
      runId: '9f3c2b1a',
    },
  })
  assert.match(isolated, /Execution isolation \(ENG-21\)/)
  assert.match(isolated, /branch agent\/eng-21\/9f3c2b1a/)
  assert.match(isolated, /approved base main@eeb68e20{33}/)
  assert.match(isolated, /never push, merge, rebase, or touch files outside this checkout/)

  const shared = buildTaskText(command(), base)
  assert.doesNotMatch(shared, /Execution isolation/)
})
