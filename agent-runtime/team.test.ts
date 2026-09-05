import assert from 'node:assert/strict'
import test from 'node:test'

import {
  DEFAULT_FORGE_TEAM,
  FORGE_FIELDS,
  FORGE_HARNESSES,
  FORGE_PLAYERS,
  listForgeTeamAssignments,
  resolveForgeAssignment,
  type ForgeTeam,
} from './team'

test('default Forge team maps one ready player to every core position', () => {
  const assignments = listForgeTeamAssignments()
  assert.deepEqual(
    assignments.map((assignment) => assignment.position),
    ['scout', 'architect', 'lead', 'smith', 'assay', 'dev_ops'],
  )
  assert.ok(assignments.every((assignment) => assignment.player.ready))
  assert.ok(assignments.every((assignment) => assignment.field.ready))
  assert.ok(assignments.every((assignment) => assignment.harness.status === 'ready'))
})

test('player roster is separate from harness selection', () => {
  assert.deepEqual(Object.keys(FORGE_PLAYERS).sort(), [
    'deepseek-flash',
    'deepseek-pro',
    'forge-deterministic-assay',
  ])
  assert.equal(FORGE_PLAYERS['deepseek-flash'].provider, 'deepseek')
  assert.equal(FORGE_PLAYERS['deepseek-pro'].provider, 'deepseek')
  assert.equal(FORGE_PLAYERS['forge-deterministic-assay'].provider, 'forge')
  assert.equal('harness' in FORGE_PLAYERS['deepseek-flash'], false)
})

test('same model may use different harnesses by mapped position', () => {
  const scout = resolveForgeAssignment('scout')
  const smith = resolveForgeAssignment('smith')
  assert.equal(scout.playerId, 'deepseek-flash')
  assert.equal(smith.playerId, 'deepseek-flash')
  assert.equal(scout.harnessId, 'forge-native')
  assert.equal(smith.harnessId, 'opencode')
})

test('default team keeps all core execution on the local sequential field', () => {
  const assignments = listForgeTeamAssignments()
  assert.ok(assignments.every((assignment) => assignment.fieldId === 'local'))
  assert.equal(FORGE_FIELDS.local.topology, 'sequential')
})

test('Warp swarm exists as a reserved field but fails closed until qualified', () => {
  assert.equal(FORGE_FIELDS['warp-swarm'].topology, 'parallel-capable')
  assert.equal(FORGE_FIELDS['warp-swarm'].ready, false)

  const team: ForgeTeam = {
    ...DEFAULT_FORGE_TEAM,
    assignments: {
      ...DEFAULT_FORGE_TEAM.assignments,
      smith: {
        ...DEFAULT_FORGE_TEAM.assignments.smith,
        fieldId: 'warp-swarm',
      },
    },
  }

  assert.throws(() => resolveForgeAssignment('smith', team), /Forge field 'warp-swarm' is not ready/)
})

test('implemented harness mappings are distinct from host runtime readiness', () => {
  assert.equal(FORGE_HARNESSES.opencode.status, 'ready')
  assert.equal(FORGE_HARNESSES['forge-assay'].status, 'ready')
  assert.equal(FORGE_HARNESSES.openclaw.status, 'ready')
  assert.equal(FORGE_HARNESSES.pi.status, 'unconfigured')
  assert.equal(FORGE_HARNESSES['warp-agent'].status, 'interactive-only')
})

test('default role model choices live only in the team map', () => {
  assert.equal(resolveForgeAssignment('scout').playerId, 'deepseek-flash')
  assert.equal(resolveForgeAssignment('architect').playerId, 'deepseek-pro')
  assert.equal(resolveForgeAssignment('dev_ops').playerId, 'deepseek-pro')
  assert.equal(resolveForgeAssignment('lead').playerId, 'deepseek-pro')
  assert.equal(resolveForgeAssignment('smith').playerId, 'deepseek-flash')
  assert.equal(resolveForgeAssignment('assay').playerId, 'forge-deterministic-assay')
})

test('Smith grade changes are map variants, not lane changes', () => {
  assert.equal(resolveForgeAssignment('smith').profile, 'builder-flash')
  assert.equal(resolveForgeAssignment('smith', DEFAULT_FORGE_TEAM, 'upgrade').profile, 'builder-plus')
  assert.equal(resolveForgeAssignment('smith', DEFAULT_FORGE_TEAM, 'emergency').profile, 'builder-emergency')
})

test('factory finding #9: player models are pinned to exact ids (never vague)', () => {
  assert.equal(FORGE_PLAYERS['deepseek-flash'].model, 'deepseek-v4-flash')
  assert.equal(FORGE_PLAYERS['deepseek-pro'].model, 'deepseek-chat')
  for (const [id, player] of Object.entries(FORGE_PLAYERS)) {
    assert.ok(player.model.trim(), `${id} has an exact model id`)
    assert.notEqual(player.model, 'pro', `${id} model is not vague`)
  }
})
