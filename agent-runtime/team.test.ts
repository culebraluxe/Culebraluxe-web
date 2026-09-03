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

test('default Forge team assigns one ready player to every core position', () => {
  const assignments = listForgeTeamAssignments()
  assert.deepEqual(
    assignments.map((assignment) => assignment.position),
    ['scout', 'architect', 'smith', 'assay'],
  )
  assert.ok(assignments.every((assignment) => assignment.player.ready))
  assert.ok(assignments.every((assignment) => assignment.field.ready))
  assert.ok(assignments.every((assignment) => assignment.harness.status === 'ready'))
})

test('current factual roster contains only DeepSeek Flash and Pro', () => {
  assert.deepEqual(Object.keys(FORGE_PLAYERS).sort(), ['deepseek-flash', 'deepseek-pro'])
  assert.equal(FORGE_PLAYERS['deepseek-flash'].provider, 'deepseek')
  assert.equal(FORGE_PLAYERS['deepseek-pro'].provider, 'deepseek')
})

test('default team keeps all execution on the local sequential field', () => {
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

  assert.throws(() => resolveForgeAssignment('smith', team), /Warp field 'warp-swarm' is not ready/)
})

test('OpenCode Pi and Warp Agent are explicit connection points, not fake ready runtimes', () => {
  assert.equal(FORGE_HARNESSES.opencode.status, 'unconfigured')
  assert.equal(FORGE_HARNESSES.pi.status, 'unconfigured')
  assert.equal(FORGE_HARNESSES['warp-agent'].status, 'interactive-only')
})

test('default positions select the intended DeepSeek players', () => {
  assert.equal(resolveForgeAssignment('scout').playerId, 'deepseek-flash')
  assert.equal(resolveForgeAssignment('architect').playerId, 'deepseek-pro')
  assert.equal(resolveForgeAssignment('smith').playerId, 'deepseek-flash')
  assert.equal(resolveForgeAssignment('assay').playerId, 'deepseek-pro')
})
