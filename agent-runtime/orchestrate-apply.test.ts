import assert from 'node:assert/strict'
import test from 'node:test'

import { WRITE_CAPABILITIES } from './lanes'
import { AgentRuntimeRegistry } from './registry'
import { readyAdapterReadiness } from './readiness'
import {
  assayFailureEvidence,
  followFinishedLane,
  hydrateBareReadyItems,
  isCleanAssayResult,
} from './orchestrate-apply'

const story = {
  architectBrief: 'Implement the existing Forge packet only.',
  goal: 'Keep Assay failure in repair.',
}

/** Complete Smith contract fixture (merged board + packet truth). */
const completeStory = {
  architectBrief: 'Build the execution-contract gate slice.',
  acceptanceCriteria: '- complete Smith contract passes unchanged\n- rejection evidence names the condition',
  assayCommands: '- pnpm exec tsx --test agent-runtime/execution-contract.test.ts',
}

/**
 * Deterministic registry for gate integration tests: builder-flash resolves to
 * a READY local adapter with full Smith capabilities (no env dependence, no
 * adapter construction during hydration).
 */
function readyBuilderRegistry(): AgentRuntimeRegistry {
  const registry = new AgentRuntimeRegistry()
  registry.registerAdapter({
    adapterId: 'gate-test',
    description: 'deterministic execution-contract test adapter',
    capabilities: WRITE_CAPABILITIES,
    readiness: () => readyAdapterReadiness('delegated', 'gate-test adapter is ready'),
    factory: () => {
      throw new Error('adapter must not be constructed during hydration')
    },
  })
  registry.registerProfile({
    profile: 'builder-flash',
    adapterId: 'gate-test',
    capabilities: WRITE_CAPABILITIES,
  })
  // V6 Lead PRE gate: hydration/follow now stamps Lead before Smith, so the
  // deterministic gate registry must also resolve the Lead profile.
  registry.registerProfile({
    profile: 'lead-pro',
    adapterId: 'gate-test',
    capabilities: WRITE_CAPABILITIES,
  })
  return registry
}

/** Same registry but the adapter is registered and NOT ready (V4-07 blocked). */
function unreadyBuilderRegistry(): AgentRuntimeRegistry {
  const registry = new AgentRuntimeRegistry()
  registry.registerAdapter({
    adapterId: 'gate-test',
    description: 'deterministic execution-contract test adapter',
    capabilities: WRITE_CAPABILITIES,
    readiness: () => ({
      registered: true,
      installed: false,
      authentication: 'delegated',
      ready: false,
      reason: 'gate-test adapter is not installed on this host',
    }),
    factory: () => {
      throw new Error('adapter must not be constructed during hydration')
    },
  })
  registry.registerProfile({
    profile: 'builder-flash',
    adapterId: 'gate-test',
    capabilities: WRITE_CAPABILITIES,
  })
  registry.registerProfile({
    profile: 'lead-pro',
    adapterId: 'gate-test',
    capabilities: WRITE_CAPABILITIES,
  })
  return registry
}

test('Assay Complete with clean tests is a clean pass', () => {
  assert.equal(
    isCleanAssayResult({
      resultStatus: 'Complete',
      testsSummary: '12 passed, 0 errors',
    }),
    true,
  )
})

test('Assay non-Complete result fails closed', () => {
  assert.equal(
    isCleanAssayResult({
      resultStatus: 'Partial',
      testsSummary: 'tests executed',
    }),
    false,
  )
})

test('Assay nominal Complete with fail violation or policy evidence fails closed', () => {
  for (const testsSummary of [
    '1 failed, 11 passed',
    'verification violation detected',
    'policy gate rejected the change',
  ]) {
    assert.equal(
      isCleanAssayResult({ resultStatus: 'Complete', testsSummary }),
      false,
      testsSummary,
    )
  }
})

test('legacy Assay evidence names configured commands without falsely calling them failed', () => {
  assert.equal(
    assayFailureEvidence({
      testsSummary: '1 failed',
      failedCommands: ['node --test agent-runtime/orchestrate-apply.test.ts', 'pnpm typecheck'],
    }),
    '1 failed | assay commands: node --test agent-runtime/orchestrate-apply.test.ts, pnpm typecheck',
  )
})

test('Assay fail never enqueues grow', async () => {
  const enqueued: unknown[] = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-01',
    finishedRole: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'policy violation',
    getStory: async () => story,
    enqueue: async (input) => {
      enqueued.push(input)
    },
  })

  assert.equal(followed, null)
  assert.deepEqual(enqueued, [])
})

test('Assay clean pass preserves current terminal follow behavior', async () => {
  const enqueued: unknown[] = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-01',
    finishedRole: 'reviewer',
    resultStatus: 'Complete',
    testsSummary: 'all checks passed',
    getStory: async () => story,
    enqueue: async (input) => {
      enqueued.push(input)
    },
  })

  assert.equal(followed, null)
  assert.deepEqual(enqueued, [])
})

test('Ready with no Neon brief and no git packet hydrates Scout, never Smith', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const stamped = await hydrateBareReadyItems({
    listItems: async () => [{
      id: 'work-1',
      storyId: 'ENG-FORGE-V3-02-NO-BRIEF',
      state: 'Ready',
      role: null,
      modelProfile: null,
      executionEnvironment: 'DEV',
      executionPolicy: 'Unattended OK',
      priority: 1,
    }],
    getStory: async () => ({ goal: 'needs architecture first' }),
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
  })

  assert.deepEqual(stamped, ['ENG-FORGE-V3-02-NO-BRIEF:scout'])
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'scout')
  assert.equal(enqueued[0]?.modelProfile, 'scout-volume')
})

test('Scout done with no brief stops: no second Scout and no Smith', async () => {
  const enqueued: unknown[] = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-03-NO-BRIEF',
    finishedRole: 'scout',
    resultStatus: 'Complete',
    getStory: async () => ({ goal: 'still needs architecture' }),
    enqueue: async (input) => {
      enqueued.push(input)
    },
    repoRoot: '/definitely/missing',
  })

  assert.equal(followed, null)
  assert.deepEqual(enqueued, [])
})

test('Scout done with a complete contract follows to Lead PRE (V6 gate, not direct Smith)', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V3-03-WITH-BRIEF',
    finishedRole: 'scout',
    resultStatus: 'Complete',
    getStory: async () => completeStory,
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
    registry: readyBuilderRegistry(),
  })

  assert.equal(followed, 'lead')
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'lead')
  assert.equal(enqueued[0]?.modelProfile, 'lead-pro')
})

test('Scout done with brief but incomplete contract follows to Lead PRE for vetting (never direct Smith)', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const followed = await followFinishedLane({
    storyId: 'ENG-FORGE-V4-08-INCOMPLETE',
    finishedRole: 'scout',
    resultStatus: 'Complete',
    getStory: async () => ({ architectBrief: 'Build only the approved slice.' }),
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
    registry: readyBuilderRegistry(),
  })

  // V6: Lead PRE vets the incomplete contract (SOLO/SMITH/HOLD); Smith is
  // never enqueued directly from Scout.
  assert.equal(followed, 'lead')
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'lead')
})

test('Ready with a complete contract hydrates Lead PRE only when the runtime is ready', async () => {
  const enqueued: Array<{
    role: string
    modelProfile: string
    executionEnvironment?: string | null
  }> = []
  const stamped = await hydrateBareReadyItems({
    listItems: async () => [{
      id: 'work-smith-1',
      storyId: 'ENG-FORGE-V4-08-COMPLETE',
      state: 'Ready',
      role: null,
      modelProfile: null,
      executionEnvironment: 'DEV',
      executionPolicy: 'Unattended OK',
      priority: 1,
    }],
    getStory: async () => completeStory,
    enqueue: async (input) => {
      enqueued.push({
        role: input.role,
        modelProfile: input.modelProfile,
        executionEnvironment: input.executionEnvironment ?? null,
      })
    },
    repoRoot: '/definitely/missing',
    registry: readyBuilderRegistry(),
  })

  assert.deepEqual(stamped, ['ENG-FORGE-V4-08-COMPLETE:lead'])
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'lead')
  assert.equal(enqueued[0]?.modelProfile, 'lead-pro')
  assert.equal(enqueued[0]?.executionEnvironment, 'DEV')
})

test('hydration stamps Lead PRE (not Smith) when acceptance criteria are missing — Lead vets the gap', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const stamped = await hydrateBareReadyItems({
    listItems: async () => [{
      id: 'work-incomplete-1',
      storyId: 'ENG-FORGE-V4-08-NO-ACCEPTANCE',
      state: 'Ready',
      role: null,
      modelProfile: null,
      executionEnvironment: 'DEV',
      executionPolicy: 'Unattended OK',
      priority: 1,
    }],
    getStory: async () => ({
      architectBrief: completeStory.architectBrief,
      assayCommands: completeStory.assayCommands,
    }),
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
    registry: readyBuilderRegistry(),
  })

  // V6: incomplete Smith contract still hydrates Lead PRE for judgment;
  // Smith is never stamped directly, so the Smith execution-contract gate
  // fires later (Lead SMITH decision), not at hydration.
  assert.deepEqual(stamped, ['ENG-FORGE-V4-08-NO-ACCEPTANCE:lead'])
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'lead')
})

test('hydration stamps Lead PRE (not Smith) when Assay commands are missing — Lead vets the gap', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const stamped = await hydrateBareReadyItems({
    listItems: async () => [{
      id: 'work-no-assay-1',
      storyId: 'ENG-FORGE-V4-08-NO-ASSAY',
      state: 'Ready',
      role: null,
      modelProfile: null,
      executionEnvironment: 'DEV',
      executionPolicy: 'Unattended OK',
      priority: 1,
    }],
    getStory: async () => ({
      architectBrief: completeStory.architectBrief,
      acceptanceCriteria: completeStory.acceptanceCriteria,
    }),
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
    registry: readyBuilderRegistry(),
  })

  assert.deepEqual(stamped, ['ENG-FORGE-V4-08-NO-ASSAY:lead'])
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'lead')
})

test('hydration still stamps Lead PRE when the Smith adapter is not ready — Smith readiness gates later launch, not Lead hydration', async () => {
  const enqueued: Array<{ role: string; modelProfile: string }> = []
  const stamped = await hydrateBareReadyItems({
    listItems: async () => [{
      id: 'work-unready-1',
      storyId: 'ENG-FORGE-V4-08-UNREADY-ADAPTER',
      state: 'Ready',
      role: null,
      modelProfile: null,
      executionEnvironment: 'DEV',
      executionPolicy: 'Unattended OK',
      priority: 1,
    }],
    getStory: async () => completeStory,
    enqueue: async (input) => {
      enqueued.push({ role: input.role, modelProfile: input.modelProfile })
    },
    repoRoot: '/definitely/missing',
    registry: unreadyBuilderRegistry(),
  })

  // V6: hydration stamps Lead PRE (judgment gate) even when the Smith adapter
  // is unready. The Smith execution-contract readiness gate fires later when
  // Lead decides SMITH, not at hydration. No builder work is enqueued here.
  assert.deepEqual(stamped, ['ENG-FORGE-V4-08-UNREADY-ADAPTER:lead'])
  assert.equal(enqueued.length, 1)
  assert.equal(enqueued[0]?.role, 'lead')
})
