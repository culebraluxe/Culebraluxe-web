import { test } from 'node:test'
import assert from 'node:assert/strict'

import { resolveLane } from './lane-policy'
import { DEFAULT_FORGE_TEAM, type ForgeTeam } from './team'

const registered = {
  hasProfile(profile: string) {
    return [
      'scout-volume',
      'architect-pro',
      'lead-pro',
      'builder-flash',
      'builder-plus',
      'builder-emergency',
      'reviewer-other',
      'verifier-mini',
    ].includes(profile)
  },
}

test('smith default profile comes from the team map', () => {
  const d = resolveLane({ lane: 'smith', registry: registered })
  assert.equal(d.ok, true)
  if (d.ok) {
    assert.equal(d.launch.role, 'builder')
    assert.equal(d.launch.modelProfile, 'builder-flash')
    assert.equal(d.launch.maxSteps, 40)
    assert.equal(d.launch.toolPolicy, 'write')
  }
})

test('same Smith lane can map to another logical profile without changing lane code', () => {
  const team: ForgeTeam = {
    ...DEFAULT_FORGE_TEAM,
    assignments: {
      ...DEFAULT_FORGE_TEAM.assignments,
      smith: {
        ...DEFAULT_FORGE_TEAM.assignments.smith,
        profile: 'builder-plus',
        playerId: 'deepseek-pro',
        harnessId: 'forge-native',
        lineage: 'deepseek-judgment',
      },
    },
  }
  const d = resolveLane({ lane: 'smith', registry: registered, team })
  assert.equal(d.ok, true)
  if (d.ok) assert.equal(d.launch.modelProfile, 'builder-plus')
})

test('smith upgrade is a mapped assignment variant', () => {
  const d = resolveLane({ lane: 'smith', smithGrade: 'upgrade', registry: registered })
  assert.equal(d.ok, true)
  if (d.ok) assert.equal(d.launch.modelProfile, 'builder-plus')
})

test('smith emergency is locked without authorization', () => {
  const d = resolveLane({ lane: 'smith', smithGrade: 'emergency', registry: registered })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'emergency-not-authorized')
})

test('smith emergency launches mapped profile when authorized', () => {
  const d = resolveLane({
    lane: 'smith',
    smithGrade: 'emergency',
    authorizeEmergency: true,
    registry: registered,
  })
  assert.equal(d.ok, true)
  if (d.ok) assert.equal(d.launch.modelProfile, 'builder-emergency')
})

test('inspector refuses without a diff', () => {
  const d = resolveLane({
    lane: 'inspector',
    session: { smithLineage: 'deepseek-volume' },
    registry: registered,
  })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'missing-diff')
})

test('inspector refuses same mapped lineage as smith', () => {
  const d = resolveLane({
    lane: 'inspector',
    session: { smithLineage: 'deepseek-judgment', hasInlineDiff: true },
    registry: registered,
  })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'same-lineage-review')
})

test('inspector launches when mapped lineage differs and diff exists', () => {
  const d = resolveLane({
    lane: 'inspector',
    session: { smithLineage: 'deepseek-volume', hasInlineDiff: true },
    registry: registered,
  })
  assert.equal(d.ok, true)
  if (d.ok) {
    assert.equal(d.launch.role, 'reviewer')
    assert.equal(d.launch.modelProfile, 'reviewer-other')
    assert.equal(d.launch.lineage, 'deepseek-judgment')
    assert.equal(d.launch.toolPolicy, 'read-only')
  }
})

test('architect refuses to tool without a scout packet when session says so', () => {
  const d = resolveLane({
    lane: 'architect',
    session: { hasScoutPacket: false },
    registry: registered,
  })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'missing-scout-packet')
})

test('architect launches plan-only after scout packet', () => {
  const d = resolveLane({
    lane: 'architect',
    session: { hasScoutPacket: true },
    registry: registered,
  })
  assert.equal(d.ok, true)
  if (d.ok) {
    assert.equal(d.launch.role, 'architect')
    assert.equal(d.launch.maxSteps, 1)
    assert.equal(d.launch.toolPolicy, 'plan-only')
  }
})

test('unregistered mapped profile fail-closes instead of inventing a default', () => {
  const d = resolveLane({
    lane: 'smith',
    registry: { hasProfile: () => false },
  })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'profile-unregistered')
})

test('preamble is additive to extra instructions', () => {
  const d = resolveLane({
    lane: 'scout',
    extraInstructions: 'Focus on db/migrations.',
    registry: registered,
  })
  assert.equal(d.ok, true)
  if (d.ok) {
    assert.match(d.launch.specialInstructions, /Lane=scout/)
    assert.match(d.launch.specialInstructions, /db\/migrations/)
  }
})

test('smith refuses when the story has no architect brief', () => {
  const d = resolveLane({
    lane: 'smith',
    session: { hasArchitectBrief: false },
    registry: registered,
  })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'missing-architect-brief')
})

test('smith launches when the neon architect brief is present', () => {
  const d = resolveLane({
    lane: 'smith',
    session: { hasArchitectBrief: true },
    registry: registered,
  })
  assert.equal(d.ok, true)
  if (d.ok) assert.equal(d.launch.modelProfile, 'builder-flash')
})

test('default Smith model choice is owned by team mapping, not lane definition', () => {
  assert.equal(DEFAULT_FORGE_TEAM.assignments.smith.profile, 'builder-flash')
  assert.equal(DEFAULT_FORGE_TEAM.assignments.smith.playerId, 'deepseek-flash')
  assert.equal(DEFAULT_FORGE_TEAM.assignments.smith.harnessId, 'opencode')
})
