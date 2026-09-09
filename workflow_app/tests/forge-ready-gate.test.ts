import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  QA_APPLICABLE_WORK_TYPES,
  storyReadyToRunReasons,
  type StoryReadyToRunFacts,
} from '../forge/forge-ready-gate'

const facts = (overrides: Partial<StoryReadyToRunFacts>): StoryReadyToRunFacts => ({
  workType: 'FEATURE',
  acceptanceCriteria: '- builds\n- verifies',
  assayCommands: '- `pnpm exec tsx --test x.test.ts`',
  ...overrides,
})

test('Ready gate: a complete QA-applicable packet may leave Planned', () => {
  assert.deepEqual(
    storyReadyToRunReasons(
      facts({ workType: 'FEATURE', acceptanceCriteria: 'a', assayCommands: '- `t`' }),
    ),
    [],
  )
  assert.deepEqual(storyReadyToRunReasons(facts({ workType: 'BUG' })), [])
  assert.deepEqual(storyReadyToRunReasons(facts({ workType: 'HOTFIX' })), [])
})

test('Ready gate: missing acceptance OR assay plan holds a QA-applicable story at Planned', () => {
  assert.deepEqual(storyReadyToRunReasons(facts({ acceptanceCriteria: '   ' })), [
    'ready-gate:missing-acceptance',
  ])
  assert.deepEqual(storyReadyToRunReasons(facts({ assayCommands: null })), [
    'ready-gate:missing-assay-plan',
  ])
  assert.deepEqual(
    storyReadyToRunReasons(facts({ assayCommands: '# placeholder' })),
    ['ready-gate:missing-assay-plan'],
  )
  const both = storyReadyToRunReasons(
    facts({ acceptanceCriteria: null, assayCommands: '   ' }),
  )
  assert.deepEqual(both, ['ready-gate:missing-acceptance', 'ready-gate:missing-assay-plan'])
})

test('Ready gate: non-QA work types are never gated by this seam', () => {
  assert.deepEqual(
    storyReadyToRunReasons(facts({ workType: 'RESEARCH', acceptanceCriteria: null })),
    [],
  )
  assert.deepEqual(
    storyReadyToRunReasons(facts({ workType: 'MIGRATION', assayCommands: null })),
    [],
  )
})

test('Ready gate: QA-applicable work types are exactly FEATURE/BUG/HOTFIX', () => {
  assert.deepEqual([...QA_APPLICABLE_WORK_TYPES].sort(), ['BUG', 'FEATURE', 'HOTFIX'])
})
