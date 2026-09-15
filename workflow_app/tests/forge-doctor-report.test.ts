import test from 'node:test'
import assert from 'node:assert/strict'

import {
  deriveBoardVsTable,
  formatAgeMs,
  isControlPlaneClear,
  renderControlPlaneReport,
  renderForgeDoctorReport,
  renderPostcard,
  renderWorkerLiveness,
  type DoctorControlPlane,
  type DoctorPostcard,
  type DoctorWorkerLiveness,
  type ForgeDoctorReportInput,
} from '../forge/forge-doctor-report'

const NOW = '2026-09-15T22:00:00.000Z'

function emptyControlPlane(): DoctorControlPlane {
  return { instances: 0, openTasks: 0, openWorkItems: 0, activeClaims: 0, oldestClaim: null }
}

function worker(overrides: Partial<DoctorWorkerLiveness> = {}): DoctorWorkerLiveness {
  return {
    status: 'alive',
    newestInvocationAt: '2026-09-15T21:59:00.000Z',
    lastFailure: null,
    invocations: 3,
    logPath: '/tmp/agent-worker.invocations.log',
    ...overrides,
  }
}

function postcard(overrides: Partial<DoctorPostcard> = {}): DoctorPostcard {
  return {
    boardCount: 0,
    tableCount: 0,
    activeDecisions: 0,
    roiWindowDays: 7,
    roiRows: [],
    newestLearnPassAt: null,
    ...overrides,
  }
}

function report(overrides: Partial<ForgeDoctorReportInput> = {}): string {
  return renderForgeDoctorReport({
    controlPlane: emptyControlPlane(),
    worker: worker(),
    postcard: postcard(),
    now: NOW,
    ...overrides,
  })
}

test('empty control plane renders CLEAR with no oldest claim', () => {
  const controlPlane = emptyControlPlane()
  assert.equal(isControlPlaneClear(controlPlane), true)
  const rendered = renderControlPlaneReport(controlPlane)
  assert.match(rendered, /CONTROL PLANE: CLEAR/)
  assert.match(rendered, /oldest claim: none/)
  assert.doesNotMatch(rendered, /oldest claim: .+ — /)
})

test('the empty-control-plane report is fully rendered and pure', () => {
  const first = report()
  const second = report()
  assert.equal(first, second)
  assert.match(first, /CONTROL PLANE: CLEAR/)
  assert.match(first, /WORKER: ALIVE/)
  assert.match(first, /board vs table: agree/)
})

test('a held claim is BUSY and the oldest claim NAMES its ledger', () => {
  const controlPlane: DoctorControlPlane = {
    instances: 4,
    openTasks: 1,
    openWorkItems: 0,
    activeClaims: 1,
    oldestClaim: { ledger: 'agent_work_item', ref: 'wi-42', ageMs: 42 * 60_000 },
  }
  const rendered = renderControlPlaneReport(controlPlane)
  assert.match(rendered, /CONTROL PLANE: BUSY/)
  assert.match(rendered, /oldest claim: 42m — agent_work_item \(wi-42\)/)
})

test('board drifted from the table reports DRIFT, not agreement', () => {
  assert.equal(deriveBoardVsTable(3, 3), 'agree')
  assert.equal(deriveBoardVsTable(3, 2), 'drift')
  const rendered = renderPostcard(postcard({ boardCount: 3, tableCount: 2 }))
  assert.match(rendered, /board vs table: DRIFT/)
  assert.match(rendered, /board 3 vs table 2/)
})

test('a failing worker reports FAILING and the most recent reason', () => {
  const failing = worker({
    status: 'failing',
    newestInvocationAt: '2026-09-15T21:00:00.000Z',
    lastFailure: 'end: exit=2 env-local-missing repo=/x',
    invocations: 463,
  })
  const rendered = renderWorkerLiveness(failing)
  assert.match(rendered, /WORKER: FAILING/)
  assert.match(rendered, /last failure: end: exit=2 env-local-missing repo=\/x/)
  assert.match(rendered, /invocations: 463/)
})

test('a missing or empty log reads as UNKNOWN/never, never a false healthy', () => {
  const unknown = worker({ status: 'unknown', newestInvocationAt: null, lastFailure: null, invocations: null })
  const rendered = renderWorkerLiveness(unknown)
  assert.match(rendered, /WORKER: UNKNOWN/)
  assert.match(rendered, /newest invocation: never/)
  assert.match(rendered, /invocations: unknown/)
  assert.doesNotMatch(rendered, /WORKER: ALIVE/)
})

test('age formatting never reports a non-finite age as zero', () => {
  assert.equal(formatAgeMs(42 * 60_000), '42m')
  assert.equal(formatAgeMs(4 * 3_600_000 + 12 * 60_000), '4h 12m')
  assert.equal(formatAgeMs(3 * 86_400_000 + 4 * 3_600_000), '3d 4h')
  assert.equal(formatAgeMs(Number.NaN), 'unknown')
})
