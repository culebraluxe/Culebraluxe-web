import { test } from 'node:test'
import assert from 'node:assert/strict'

import { DEFAULT_LANES } from './lanes'
import { resolveLane } from './lane-policy'

const registered = {
  hasProfile(profile: string) {
    return [
      'scout-volume',
      'architect-pro',
      'builder-flash',
      'builder-plus',
      'builder-emergency',
      'reviewer-other',
      'verifier-mini',
    ].includes(profile)
  },
}

test('smith default stays on builder-flash', () => {
  const d = resolveLane({ lane: 'smith', registry: registered })
  assert.equal(d.ok, true)
  if (d.ok) {
    assert.equal(d.launch.role, 'builder')
    assert.equal(d.launch.modelProfile, 'builder-flash')
    assert.equal(d.launch.maxSteps, 40)
    assert.equal(d.launch.toolPolicy, 'write')
  }
})

test('smith upgrade flips profile only for that session', () => {
  const d = resolveLane({ lane: 'smith', smithGrade: 'upgrade', registry: registered })
  assert.equal(d.ok, true)
  if (d.ok) assert.equal(d.launch.modelProfile, 'builder-plus')
})

test('smith emergency is locked without authorization', () => {
  const d = resolveLane({ lane: 'smith', smithGrade: 'emergency', registry: registered })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'emergency-not-authorized')
})

test('smith emergency launches when authorized', () => {
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
    session: { smithLineage: 'volume-lab' },
    registry: registered,
  })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'missing-diff')
})

test('inspector refuses same lineage as smith', () => {
  const d = resolveLane({
    lane: 'inspector',
    session: { smithLineage: 'judgment-lab', hasInlineDiff: true },
    registry: registered,
  })
  assert.equal(d.ok, false)
  if (!d.ok) assert.equal(d.code, 'same-lineage-review')
})

test('inspector launches when lineage differs and diff exists', () => {
  const d = resolveLane({
    lane: 'inspector',
    session: { smithLineage: 'volume-lab', hasInlineDiff: true },
    registry: registered,
  })
  assert.equal(d.ok, true)
  if (d.ok) {
    assert.equal(d.launch.role, 'reviewer')
    assert.equal(d.launch.modelProfile, 'reviewer-other')
    assert.equal(d.launch.lineage, 'judgment-lab')
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

test('unregistered profile fail-closes instead of inventing a default', () => {
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

test('default smith binding stays the live factory profile', () => {
  assert.equal(DEFAULT_LANES.smith.profile, 'builder-flash')
})
