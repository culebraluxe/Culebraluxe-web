import assert from 'node:assert/strict'
import test from 'node:test'

import type { AgentCapability } from './capabilities'
import { WRITE_CAPABILITIES } from './lanes'
import {
  blockedAdapterReadiness,
  readyAdapterReadiness,
  type AdapterReadiness,
} from './readiness'
import type { StoryPacketFields } from './story-session'
import {
  executionContractFailureText,
  validateExecutionContract,
  type ExecutionContractRegistry,
  type ExecutionContractSubject,
} from './execution-contract'

const COMPLETE_STORY: StoryPacketFields = {
  architectBrief: 'Implement the approved packet slice only.',
  acceptanceCriteria:
    '- a fully complete Smith contract passes unchanged\n- rejection evidence names the failing condition',
  assayCommands: '- pnpm exec tsx --test agent-runtime/execution-contract.test.ts',
}

function fakeRegistry(overrides?: {
  registered?: boolean
  readiness?: AdapterReadiness
  capabilities?: AgentCapability[]
  resolveProfileThrows?: boolean
}): ExecutionContractRegistry {
  const profile = 'builder-flash'
  const registered = overrides?.registered ?? true
  const readiness =
    overrides?.readiness ?? readyAdapterReadiness('delegated', 'adapter ready for tests')
  const capabilities = overrides?.capabilities ?? WRITE_CAPABILITIES
  return {
    hasProfile: (candidate) => registered && candidate === profile,
    inspectProfileReadiness: () => readiness,
    resolveProfile: () => {
      if (overrides?.resolveProfileThrows) {
        throw new Error('resolveProfile must not be reached for an unregistered profile')
      }
      return { capabilities }
    },
  }
}

function subject(
  overrides: Partial<ExecutionContractSubject> = {},
): ExecutionContractSubject {
  return {
    story: COMPLETE_STORY,
    executionTarget: 'DEV',
    modelProfile: 'builder-flash',
    registry: fakeRegistry(),
    field: { id: 'local', ready: true },
    ...overrides,
  }
}

test('a fully complete Smith contract passes unchanged', () => {
  const storyBefore = JSON.stringify(COMPLETE_STORY)
  const result = validateExecutionContract(subject())
  assert.equal(result.ok, true)
  assert.equal(JSON.stringify(COMPLETE_STORY), storyBefore, 'story packet is not mutated')
  assert.equal(executionContractFailureText(result), null)
})

test('missing Architect brief rejects Smith with named evidence', () => {
  const result = validateExecutionContract(
    subject({ story: { ...COMPLETE_STORY, architectBrief: null } }),
  )
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.code, 'missing-architect-brief')
})

test('missing acceptance criteria rejects Smith before external execution', () => {
  const result = validateExecutionContract(
    subject({ story: { ...COMPLETE_STORY, acceptanceCriteria: null } }),
  )
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.code, 'missing-acceptance-criteria')
})

test('missing Assay commands do not block Smith launch', () => {
  for (const assayCommands of [null, '   \n  ', '']) {
    const result = validateExecutionContract(
      subject({ story: { ...COMPLETE_STORY, assayCommands } }),
    )
    assert.equal(result.ok, true, `assayCommands=${JSON.stringify(assayCommands)}`)
  }
})

test('missing execution target rejects Smith before external execution', () => {
  for (const executionTarget of [undefined, null, '  ']) {
    const result = validateExecutionContract(subject({ executionTarget }))
    assert.equal(result.ok, false, `executionTarget=${JSON.stringify(executionTarget)}`)
    if (result.ok) continue
    assert.equal(result.code, 'missing-execution-target')
  }
})

test('invalid execution target rejects Smith before external execution', () => {
  for (const executionTarget of ['PROD-X', 'STAGING', 'DEV/PROD']) {
    const result = validateExecutionContract(subject({ executionTarget }))
    assert.equal(result.ok, false, executionTarget)
    if (result.ok) continue
    assert.equal(result.code, 'invalid-execution-target')
  }
})

test('unknown profile rejects Smith and never falls back to another profile', () => {
  const result = validateExecutionContract(
    subject({ modelProfile: 'builder-mystery', registry: fakeRegistry({ registered: false }) }),
  )
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.code, 'profile-unregistered')
})

test('unregistered profile never invokes profile resolution (no throw path)', () => {
  const result = validateExecutionContract(
    subject({
      modelProfile: 'builder-mystery',
      registry: fakeRegistry({ registered: false, resolveProfileThrows: true }),
    }),
  )
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.code, 'profile-unregistered')
})

test('registered-but-unready adapter rejects Smith under the readiness gate', () => {
  const result = validateExecutionContract(
    subject({
      registry: fakeRegistry({
        readiness: blockedAdapterReadiness({
          installed: false,
          authentication: 'delegated',
          reason: 'harness entrypoint not found: /nope',
        }),
      }),
    }),
  )
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.code, 'adapter-not-ready')
})

test('unavailable Field rejects Smith without falling back', () => {
  const fieldResult = validateExecutionContract(
    subject({ field: { id: 'warp-swarm', ready: false } }),
  )
  assert.equal(fieldResult.ok, false)
  if (fieldResult.ok) return
  assert.equal(fieldResult.code, 'field-not-ready')

  const missingResult = validateExecutionContract(subject({ field: null }))
  assert.equal(missingResult.ok, false)
  if (missingResult.ok) return
  assert.equal(missingResult.code, 'field-not-ready')
})

test('insufficient Smith capabilities reject Smith with the missing capability named', () => {
  const withoutCommit = WRITE_CAPABILITIES.filter((c) => c !== 'git.commit')
  const result = validateExecutionContract(
    subject({ registry: fakeRegistry({ capabilities: withoutCommit }) }),
  )
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.code, 'insufficient-capabilities')
})

test('multiple launch-time failures are all reported', () => {
  const result = validateExecutionContract(
    subject({
      story: { acceptanceCriteria: null, assayCommands: null },
      executionTarget: undefined,
    }),
  )
  assert.equal(result.ok, false)
  if (result.ok) return
  assert.equal(result.code, 'missing-architect-brief')
  const codes = result.reasons.map((r) => r.code)
  assert.ok(codes.includes('missing-architect-brief'))
  assert.ok(codes.includes('missing-acceptance-criteria'))
  assert.ok(codes.includes('missing-execution-target'))
  assert.equal(codes.includes('missing-assay-plan' as never), false)
})

test('failure text is a compact provider-neutral evidence line', () => {
  const result = validateExecutionContract(
    subject({
      story: { ...COMPLETE_STORY, acceptanceCriteria: null },
      field: { id: 'local', ready: false },
    }),
  )
  const text = executionContractFailureText(result)
  assert.ok(text)
  assert.match(text!, /missing-acceptance-criteria/)
  assert.match(text!, /field-not-ready/)
  assert.ok(!/deepseek|warp|openclaw|claude|gpt|kimi|mistral/i.test(text!))
})
