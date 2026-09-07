import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  buildRepoContextQuery,
  latestScoutResearch,
  runRepoContextTaskPacket,
  withRepoContextPacket,
  withScoutResearch,
} from './repo-context'
import type { StoryRun } from '../db/storyboard'

function run(overrides: Partial<StoryRun>): StoryRun {
  return {
    id: 'run-1',
    storyId: 'ENG-1',
    startedAt: '2026-09-06T00:00:00.000Z',
    endedAt: null,
    resultStatus: 'Complete',
    runType: null,
    agentRuntime: null,
    completion: 100,
    notes: null,
    commitHash: null,
    testsSummary: null,
    executionEnvironment: 'DEV',
    goalSnapshot: null,
    preconditionsSnapshot: null,
    architectBriefSnapshot: null,
    contextRefsSnapshot: null,
    acceptanceCriteriaSnapshot: null,
    postconditionsSnapshot: null,
    createdAt: '2026-09-06T00:00:00.000Z',
    updatedAt: '2026-09-06T00:00:00.000Z',
    ...overrides,
  }
}

test('Ripwire packet uses bounded read-only repository arguments', () => {
  let called: { file: string; args: readonly string[]; cwd: string } | null = null
  const packet = runRepoContextTaskPacket({
    workspace: '/repo',
    task: 'ENG-1: investigate assay repair',
    bin: '/usr/local/bin/ripwire',
    execFile: (file, args, options) => {
      called = { file, args, cwd: options.cwd }
      return '<ctx>ok</ctx>'
    },
  })

  assert.equal(packet, '<ctx>ok</ctx>')
  assert.deepEqual(called, {
    file: '/usr/local/bin/ripwire',
    cwd: '/repo',
    args: [
      '.',
      '--exclude=.next',
      '--exclude=node_modules',
      '--exclude=.ripwire-output',
      '--pack-task=ENG-1: investigate assay repair',
    ],
  })
})

test('Ripwire failure never blocks Scout execution', () => {
  const packet = runRepoContextTaskPacket({
    workspace: '/repo',
    task: 'investigate',
    execFile: () => {
      throw new Error('ripwire missing')
    },
  })
  assert.equal(packet, null)
})

test('repo context query is story-shaped and bounded', () => {
  const query = buildRepoContextQuery({
    id: 'ENG-2',
    title: 'Scout repository context',
    goal: 'Find the real seams',
    scope: 'Forge only',
    architectBrief: 'Use current runtime path',
  })
  assert.match(query, /^ENG-2: Scout repository context/)
  assert.match(query, /Goal: Find the real seams/)
  assert.match(query, /Scope: Forge only/)
})

test('Scout packet preserves lane instructions and requires durable synthesis', () => {
  const value = withRepoContextPacket('Lane=scout', '<ctx>ranked symbols</ctx>')
  assert.match(value ?? '', /^Lane=scout/)
  assert.match(value ?? '', /Ripwire structural evidence/)
  assert.match(value ?? '', /SCOUT_RESEARCH:/)
  assert.match(value ?? '', /persisted to the Story Run in Neon/)
})

test('latest Scout research prefers structured research section', () => {
  const research = latestScoutResearch([
    run({
      runType: 'scout',
      notes: 'DeepSeek Harness run completed.\n\nAssistant output:\nSummary\n\nSCOUT_RESEARCH:\n- followFinishedLane',
    }),
  ])
  assert.equal(research, 'SCOUT_RESEARCH:\n- followFinishedLane')
})

test('downstream instructions preserve their own packet and add Scout evidence', () => {
  const value = withScoutResearch(
    'Lane=architect',
    'SCOUT_RESEARCH:\n- candidate seam',
  )
  assert.match(value ?? '', /^Lane=architect/)
  assert.match(value ?? '', /Prior Scout research from the durable Story Run/)
  assert.match(value ?? '', /candidate seam/)
})
