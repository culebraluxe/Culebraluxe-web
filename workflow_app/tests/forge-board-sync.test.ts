import test from 'node:test'
import assert from 'node:assert/strict'

import {
  assertForgeExecutionTarget,
  classifyShipCommits,
  deriveBoardSync,
  parseShipCommits,
  summarizeRunEnvironments,
  appendNote,
  ForgeEnvironmentError,
  type RunEvidence,
  type ShipEvidence,
} from '../forge/forge-board-sync'

const NOW = '2026-09-11T12:00:00.000Z'

function ship(overrides: Partial<ShipEvidence> = {}): ShipEvidence {
  return { storyId: 'PROJECTS-WORKSPACE-12', commits: [], ...overrides }
}

function runs(overrides: Partial<RunEvidence> = {}): RunEvidence {
  return { runCount: 0, itemCount: 0, environments: [], ...overrides }
}

// --- criterion 3: Forge executes against PROD; anything else fails closed ----

test('guard: PROD passes', () => {
  assert.equal(assertForgeExecutionTarget('PROD'), 'PROD')
  assert.equal(assertForgeExecutionTarget('production'), 'PROD')
})

test('guard: DEV fails closed (the 2026-09-11 rule)', () => {
  assert.throws(() => assertForgeExecutionTarget('DEV'), ForgeEnvironmentError)
  assert.throws(() => assertForgeExecutionTarget('development'), ForgeEnvironmentError)
})

test('guard: an unknown or absent target fails closed rather than defaulting', () => {
  assert.throws(() => assertForgeExecutionTarget(null), ForgeEnvironmentError)
  assert.throws(() => assertForgeExecutionTarget(undefined), ForgeEnvironmentError)
  assert.throws(() => assertForgeExecutionTarget('staging'), ForgeEnvironmentError)
})

test('guard: a test context may opt in explicitly, and only that value', () => {
  assert.equal(assertForgeExecutionTarget('TEST', { allowEnvironment: 'TEST' }), 'TEST')
  assert.throws(() => assertForgeExecutionTarget('DEV', { allowEnvironment: 'TEST' }), ForgeEnvironmentError)
})

// --- criterion 1: shipped work derives Complete ------------------------------

test('a story with merged commits but no PROD run rows reconciles to Complete', () => {
  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-12', status: 'Planned', completion: 30 },
    ship: ship({ commits: ['abc1234 feat(projects): PROJECTS-WORKSPACE-12 cockpit'] }),
    runs: runs(),
    now: NOW,
  })
  assert.equal(decision.action, 'complete')
  assert.equal(decision.completion, 100)
  assert.equal(decision.reason, 'shipped')
  assert.ok(decision.note)
})

// --- criterion 4: absent evidence is stated, never estimated -----------------

test('the note says PROD run evidence is ABSENT and invents no number', () => {
  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-12', status: 'Planned', completion: 30 },
    ship: ship({ commits: ['abc1234 feat(projects): PROJECTS-WORKSPACE-12 cockpit'] }),
    runs: runs(),
    now: NOW,
  })
  assert.match(decision.note!, /PROD run evidence: ABSENT \(0 storyboard_story_run rows, 0 agent_work_item rows\)/)
  assert.match(decision.note!, /no number on this row was estimated/)
  assert.match(decision.note!, /abc1234 feat\(projects\): PROJECTS-WORKSPACE-12 cockpit/)
  // The pre-existing planning estimate must not appear as if it were measured.
  assert.doesNotMatch(decision.note!, /completion\s*30/)
})

test('the note reports PRESENT evidence with counts when runs exist', () => {
  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-12', status: 'In Progress', completion: 60 },
    ship: ship({ commits: ['abc1234 feat(projects): PROJECTS-WORKSPACE-12 cockpit'] }),
    runs: runs({ runCount: 3, itemCount: 4, environments: ['PROD', 'PROD', 'PROD'] }),
    now: NOW,
  })
  assert.equal(decision.action, 'complete')
  assert.match(decision.note!, /PROD run evidence: PRESENT \(3 run\(s\), 4 work item\(s\)/)
  assert.match(decision.note!, /environments PROD=3/)
})

// --- criterion 3b: a non-PROD run can never masquerade as PROD evidence ------

test('a run outside PROD raises a warning instead of passing silently', () => {
  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-12', status: 'Planned', completion: 0 },
    ship: ship({ commits: ['abc1234 feat(projects): PROJECTS-WORKSPACE-12 cockpit'] }),
    runs: runs({ runCount: 2, itemCount: 2, environments: ['DEV', 'DEV'] }),
    now: NOW,
  })
  assert.ok(decision.environmentWarning)
  assert.match(decision.environmentWarning!, /outside PROD: DEV/)
  assert.match(decision.note!, /WARNING: Forge run\(s\) executed outside PROD: DEV/)
})

test('unrecorded environments are reported as UNKNOWN, not assumed to be PROD', () => {
  const summary = summarizeRunEnvironments([null, undefined, '', 'PROD'])
  assert.equal(summary.unknownCount, 3)
  assert.equal(summary.byEnvironment.PROD, 1)
  assert.equal(summary.byEnvironment.UNKNOWN, 3)
  assert.deepEqual(summary.nonProdEnvironments, [])
})

// --- criterion 7: a story with neither stays Planned -------------------------

test('a story with neither commits nor runs stays Planned', () => {
  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-16', status: 'Planned', completion: 0 },
    ship: ship({ storyId: 'PROJECTS-WORKSPACE-16' }),
    runs: runs(),
    now: NOW,
  })
  assert.equal(decision.action, 'no-change')
  assert.equal(decision.reason, 'no-ship-evidence')
  assert.equal(decision.note, null)
})

test('a story with runs but no shipped commits is left alone', () => {
  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-13', status: 'In Progress', completion: 25 },
    ship: ship({ storyId: 'PROJECTS-WORKSPACE-13' }),
    runs: runs({ runCount: 2, itemCount: 2, environments: ['PROD', 'PROD'] }),
    now: NOW,
  })
  assert.equal(decision.action, 'no-change')
  assert.equal(decision.reason, 'run-evidence-only')
  assert.equal(decision.note, null)
})

// --- criterion 5: idempotent and safe to re-run ------------------------------

test('a story already Complete is never re-noted (idempotent)', () => {
  const first = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-12', status: 'Planned', completion: 30 },
    ship: ship({ commits: ['abc1234 feat(projects): PROJECTS-WORKSPACE-12 cockpit'] }),
    runs: runs(),
    now: NOW,
  })
  assert.equal(first.action, 'complete')

  const second = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-12', status: 'Complete', completion: 100 },
    ship: ship({ commits: ['abc1234 feat(projects): PROJECTS-WORKSPACE-12 cockpit'] }),
    runs: runs(),
    now: NOW,
  })
  assert.equal(second.action, 'no-change')
  assert.equal(second.reason, 'already-complete')
  assert.equal(second.note, null)
})

test('appendNote preserves existing history', () => {
  assert.equal(appendNote(null, 'note'), 'note')
  assert.equal(appendNote('', 'note'), 'note')
  assert.equal(appendNote('original\n', 'note'), 'original\n\nnote')
})

// --- ship evidence parsing ---------------------------------------------------

test('parseShipCommits matches the story id case-insensitively', () => {
  const log = [
    'abc1234 feat(projects): PROJECTS-WORKSPACE-12 cockpit',
    'def5678 integrate(projects-workspace-10)',
    '9999999 chore: unrelated tidy-up',
  ].join('\n')

  const matches = parseShipCommits('PROJECTS-WORKSPACE-12', log)
  assert.equal(matches.length, 1)
  assert.match(matches[0], /^abc1234 /)

  assert.equal(parseShipCommits('projects-workspace-10', log).length, 1)
  assert.equal(parseShipCommits('PROJECTS-WORKSPACE-99', log).length, 0)
  assert.equal(parseShipCommits('', log).length, 0)
})

// --- a packet commit is not shipped work (found on live data) ----------------

test('a docs/packet commit alone does NOT complete a story', () => {
  const log = '1299d4e docs(forge): story packet for PROJECTS-WORKSPACE-13'
  const { shipping, docsOnly } = classifyShipCommits('PROJECTS-WORKSPACE-13', log)
  assert.deepEqual(shipping, [])
  assert.equal(docsOnly.length, 1)

  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-13', status: 'Planned', completion: 25 },
    ship: { storyId: 'PROJECTS-WORKSPACE-13', commits: shipping, docsOnly },
    runs: runs({ runCount: 2, itemCount: 2, environments: ['DEV', 'DEV'] }),
    now: NOW,
  })
  assert.equal(decision.action, 'no-change')
  assert.equal(decision.reason, 'packet-only')
  assert.equal(decision.note, null)
})

test('a shipping commit alongside docs commits still completes, and says what it excluded', () => {
  const log = [
    'aaa1111 feat(projects): PROJECTS-WORKSPACE-12 build the cockpit',
    'bbb2222 docs(forge): story packet for PROJECTS-WORKSPACE-12',
  ].join('\n')
  const { shipping, docsOnly } = classifyShipCommits('PROJECTS-WORKSPACE-12', log)
  assert.equal(shipping.length, 1)
  assert.equal(docsOnly.length, 1)

  const decision = deriveBoardSync({
    story: { id: 'PROJECTS-WORKSPACE-12', status: 'Planned', completion: 40 },
    ship: { storyId: 'PROJECTS-WORKSPACE-12', commits: shipping, docsOnly },
    runs: runs(),
    now: NOW,
  })
  assert.equal(decision.action, 'complete')
  assert.match(decision.note!, /Excluded 1 docs\/packet commit\(s\)/)
})

test('feat, fix and test commits all count as shipping evidence', () => {
  const log = [
    'aaa1111 feat(x): PROJECTS-WORKSPACE-12 a',
    'bbb2222 fix(x): PROJECTS-WORKSPACE-12 b',
    'ccc3333 test(x): PROJECTS-WORKSPACE-12 c',
  ].join('\n')
  assert.equal(classifyShipCommits('PROJECTS-WORKSPACE-12', log).shipping.length, 3)
})

// --- whole-token matching: OPS-11 must not match OPS-11A ----------------------

test('a story id only matches as a whole token, not as a prefix of another id', () => {
  const log = 'd4bf446 feat(ops): OPS-11A — Operational Issue Queue + Runbook dashboard'

  // OPS-11A's commit must NOT complete OPS-11.
  assert.equal(parseShipCommits('OPS-11', log).length, 0)
  assert.equal(parseShipCommits('OPS-11A', log).length, 1)
  assert.equal(classifyShipCommits('OPS-11', log).shipping.length, 0)
})

test('an id still matches when surrounded by punctuation', () => {
  const log = [
    'abc1234 integrate(projects-workspace-10)',
    'def5678 feat(x): PROJECTS-WORKSPACE-12.',
  ].join('\n')
  assert.equal(parseShipCommits('projects-workspace-10', log).length, 1)
  assert.equal(parseShipCommits('PROJECTS-WORKSPACE-12', log).length, 1)
})

// --- the packet rules, as observed on live data ------------------------------

test('a packet commit with no conventional type is not shipping evidence', () => {
  const log = 'd8df09a ENG-FORGE-V5-11: lead dev qa topology packet'
  const { shipping, docsOnly } = classifyShipCommits('ENG-FORGE-V5-11', log)
  assert.deepEqual(shipping, [])
  assert.equal(docsOnly.length, 1)
})

test('a chore(packet + seed) commit is not shipping evidence', () => {
  const log = '2c39c39 chore(forge): ENG-PROJECTS-ANCHOR-02 packet + seed for the LEAD-routing validation run'
  const { shipping, docsOnly } = classifyShipCommits('ENG-PROJECTS-ANCHOR-02', log)
  assert.deepEqual(shipping, [])
  assert.equal(docsOnly.length, 1)
})

test('a real feat commit beside a packet commit still ships', () => {
  const log = [
    'fc3887b feat(forge): ENG-FORGE-V5-08 Forge Consistency Janitor (read-only audit)',
    '9905816 ENG-FORGE-V5-08: Forge consistency janitor packet',
  ].join('\n')
  const { shipping, docsOnly } = classifyShipCommits('ENG-FORGE-V5-08', log)
  assert.equal(shipping.length, 1)
  assert.equal(docsOnly.length, 1)
})

// --- a human Hold is not repealed by shipped code ----------------------------

test('a Hold is reported as held-shipped, not silently completed', () => {
  const decision = deriveBoardSync({
    story: { id: 'ENG-FORGE-SHAPE-01', status: 'Hold', completion: 0 },
    ship: ship({
      storyId: 'ENG-FORGE-SHAPE-01',
      commits: ['cd5740d feat(forge): ENG-FORGE-SHAPE-01 shaping layer'],
    }),
    runs: runs(),
    now: NOW,
  })
  assert.equal(decision.action, 'no-change')
  assert.equal(decision.reason, 'held-shipped')
  assert.equal(decision.note, null)
})

test('Deferred is protected the same way as Hold', () => {
  const decision = deriveBoardSync({
    story: { id: 'ENG-FORGE-V5-12', status: 'Deferred', completion: 0 },
    ship: ship({
      storyId: 'ENG-FORGE-V5-12',
      commits: ['c4465ef feat(forge): ENG-FORGE-V5-12 decomposition contracts'],
    }),
    runs: runs(),
    now: NOW,
  })
  assert.equal(decision.action, 'no-change')
  assert.equal(decision.reason, 'held-shipped')
})

test('an active story that shipped still completes', () => {
  const decision = deriveBoardSync({
    story: { id: 'OPS-11A', status: 'Planned', completion: 0 },
    ship: ship({ storyId: 'OPS-11A', commits: ['d4bf446 feat(ops): OPS-11A Operational Issue Queue'] }),
    runs: runs(),
    now: NOW,
  })
  assert.equal(decision.action, 'complete')
  assert.equal(decision.completion, 100)
})
