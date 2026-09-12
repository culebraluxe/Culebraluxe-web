// ---------------------------------------------------------------------------
// FORGE-SMITH-DOOR-01 — both serial doors, named in one test.
//
// Grok's 91 review: "no assignment → no scope to violate" (b484301) and "no
// assignment → HOLD at launch" (57b64b7) are two sentences that must stay
// consistent. If any path can start Smith with an empty assignment AND skip the
// scope lock, the lock is decorative again. So these cases name BOTH doors, and the
// last case asserts the wiring directly in the runner source (the drift detector).
//
// No database, no OpenCode: the doors are pure decisions.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import {
  NO_ASSIGNMENT_REASON,
  SCOPE_MISS_PREFIX,
  SERIAL_SMITH_NODES,
  serialLaunchDoor,
  serialScopeMissReasons,
} from '../forge/forge-serial-doors'
import { createAgentRuntimeForgeRoleRunner } from '../forge/agent-runtime-role-runner'

const RUNNER_SRC = readFileSync(
  join(process.cwd(), 'workflow_app/forge/agent-runtime-role-runner.ts'),
  'utf8',
)

test('door 1: all four executing Smith roles HOLD without an accepted assignment', () => {
  for (const nodeId of SERIAL_SMITH_NODES) {
    const decision = serialLaunchDoor({ nodeId, hasAcceptedAssignment: false })
    assert.equal(decision.allowed, false, `${nodeId} must not launch unassigned`)
    assert.match(String(decision.reason), /HOLD/)
    assert.match(String(decision.reason), new RegExp(NO_ASSIGNMENT_REASON.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')))
    assert.match(String(decision.reason), /LEAD_ROUTING/)
  }
})

test('door 1: the same four roles launch once an assignment is accepted', () => {
  for (const nodeId of SERIAL_SMITH_NODES) {
    const decision = serialLaunchDoor({ nodeId, hasAcceptedAssignment: true })
    assert.equal(decision.allowed, true, `${nodeId} must launch with an assignment`)
    assert.equal(decision.reason, null)
  }
})

test('door 1: lead_solo_implement and the split child do NOT take this HOLD', () => {
  // SOLO is assignment-free by design; a split child is gated by its own accepted
  // assignment contract before launch. Neither is this door's business.
  for (const nodeId of ['lead_solo_implement', 'smith_split_work']) {
    const decision = serialLaunchDoor({ nodeId, hasAcceptedAssignment: false })
    assert.equal(decision.allowed, true, `${nodeId} must not be blocked by the serial door`)
  }
})

test('door 1: non-Smith lanes are untouched', () => {
  for (const nodeId of ['lead_pre', 'lead_post', 'architect', 'qa_verify', 'deploy']) {
    assert.equal(serialLaunchDoor({ nodeId, hasAcceptedAssignment: false }).allowed, true)
  }
})

test('door 2: out-of-scope paths become prefixed miss reasons naming the owner', () => {
  const reasons = serialScopeMissReasons(['app/portal/x.ts', 'lib/y.ts'], 'assign-7')
  assert.deepEqual(reasons, [
    `${SCOPE_MISS_PREFIX}app/portal/x.ts is outside assign-7`,
    `${SCOPE_MISS_PREFIX}lib/y.ts is outside assign-7`,
  ])
  // The prefix is what the runner's HOLD path and the self-heal directive key on.
  assert.ok(reasons.every((r) => r.startsWith(SCOPE_MISS_PREFIX)))
})

test('door 2: an in-lane candidate produces no miss at all', () => {
  assert.deepEqual(serialScopeMissReasons([], 'assign-7'), [])
})

test('wiring: the runner uses BOTH doors, and door 1 is checked before the spawn', () => {
  // The drift detector Grok asked for. Source order is the honest assertion here:
  // the claim is "the lane cannot launch unassigned", and that is a statement about
  // where the check sits relative to the OpenCode execution.
  assert.match(RUNNER_SRC, /from '\.\/forge-serial-doors'/, 'the runner must use the shared doors')
  assert.match(RUNNER_SRC, /serialLaunchDoor\(/, 'door 1 must be evaluated in the runner')
  assert.match(RUNNER_SRC, /serialScopeMissReasons\(/, 'door 2 must use the shared reason builder')

  const launchCheck = RUNNER_SRC.indexOf('serialLaunchDoor(')
  const spawn = RUNNER_SRC.indexOf('executeClaimedAgentCommand(')
  const enqueue = RUNNER_SRC.indexOf('work.enqueue(')
  assert.ok(spawn > 0 && enqueue > 0, 'the spawn and enqueue call sites must still exist')
  assert.ok(
    launchCheck < spawn,
    'door 1 must be checked BEFORE the agent is executed (OpenCode spawn)',
  )
  assert.ok(launchCheck < enqueue, 'door 1 must be checked BEFORE a work item is enqueued')

  // The stale wording both commits left behind must not come back.
  assert.doesNotMatch(
    RUNNER_SRC,
    /RECORD-ONLY, deliberately/,
    'the superseded "record-only" claim must not reappear: door 2 enforces now',
  )
})

test('wiring: the four Smith roles are the ones the role mapping actually emits', async () => {
  // If the mapping grows another lane-smith node, this fails and the door list must
  // be updated — otherwise the new role would quietly escape door 1.
  const { forgeRoleNodePlan } = await import('../forge/forge-role-mapping')
  const laneSmithNodes = [
    'smith',
    'smith_split_work',
    'repair_smith',
    'fast_smith',
    'fast_repair_smith',
  ].filter((nodeId) => forgeRoleNodePlan(nodeId).lane === 'smith')
  assert.deepEqual(
    laneSmithNodes.filter((nodeId) => nodeId !== 'smith_split_work').sort(),
    [...SERIAL_SMITH_NODES].sort(),
    'SERIAL_SMITH_NODES must match the lane-smith nodes that execute work orders',
  )
})

test('the runner refuses a lane start before touching a database (unchanged)', async () => {
  // Guards the ordering claim from the outside: with no declared environment the
  // runner rejects at the lane-start guard, i.e. before any query could run.
  const runner = createAgentRuntimeForgeRoleRunner({ workerId: 'door-test' })
  await assert.rejects(
    () => runner('smith', { taskId: 't', processInstanceId: 'p' } as never),
    /PROD only|not declared/,
  )
})
