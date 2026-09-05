import assert from 'node:assert/strict'
import test from 'node:test'

import type { ProcessGraph } from '../../workflow_engine/lib/workflow/types'
import { DEAL_SET_STAGE_CLOSED } from '../command-types'
import {
  reCommandIsRouted,
  validateApplicationContract,
} from '../definitions/application-contract'
import { forgeCommandIsRouted } from '../forge-command-types'
import { parseForgeSdlc } from '../definitions/forge-sdlc'
import { parseReSupermodel } from '../definitions/re-supermodel'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — clean A/B fork of the engine's two models.
//
// The workflow engine hosts two models with separate command domains:
//   A) RE_supermodel  -> real-estate command inventory (workflow_app/command-types)
//   B) FORGE_SDLC     -> Forge command inventory (workflow_app/forge-command-types)
//
// FORGE_SDLC must validate against the FORGE inventory ONLY; an RE command
// name inside FORGE_SDLC must fail closed, and a forge.* command must never
// ride RE's router. RE validation must remain byte-for-byte unchanged.
//
// No database, no packages.
// ---------------------------------------------------------------------------

/** A minimal graph with one command-node referencing an RE business command. */
function graphWithCommand(commandType: string): ProcessGraph {
  return {
    startNodeId: 'start',
    nodes: {
      start: { id: 'start', type: 'start', transitions: [{ name: 'go', to: 'cmd' }] },
      cmd: { id: 'cmd', type: 'command', commandType, transitions: [{ name: 'done', to: 'end' }] },
      end: { id: 'end', type: 'end' },
    },
  } as unknown as ProcessGraph
}

test('ENG-FORGE-V9: FORGE_SDLC-v1 still validates through the Forge inventory', () => {
  const parsed = parseForgeSdlc() // throws if any layer fails under forgeCommandIsRouted
  assert.equal(parsed.key, 'FORGE_SDLC')
})

test('ENG-FORGE-V9: RE_supermodel still validates through the RE inventory (regression)', () => {
  const parsed = parseReSupermodel() // throws if any layer fails under the default RE inventory
  assert.equal(parsed.key, 'RE_supermodel')
})

test('ENG-FORGE-V9: an RE command inside FORGE_SDLC fails closed (A/B separation)', () => {
  const result = validateApplicationContract(
    graphWithCommand(DEAL_SET_STAGE_CLOSED),
    forgeCommandIsRouted,
  )
  assert.equal(result.valid, false, 'RE commands must not be routable in the Forge domain')
  assert.ok(
    result.errors.some((e) => e.includes(DEAL_SET_STAGE_CLOSED)),
    'error must name the unrouted command',
  )
})

test('ENG-FORGE-V9: the same RE command is routable in the RE domain (unchanged)', () => {
  const result = validateApplicationContract(
    graphWithCommand(DEAL_SET_STAGE_CLOSED),
    reCommandIsRouted,
  )
  assert.equal(result.valid, true)
  assert.deepEqual(result.errors, [])
})

test('ENG-FORGE-V9: the default application-contract inventory is RE (no RE behavior change)', () => {
  const defaulted = validateApplicationContract(graphWithCommand(DEAL_SET_STAGE_CLOSED))
  assert.equal(defaulted.valid, true, 'default inventory must remain the RE registry')
})

test('ENG-FORGE-V9: Forge inventory is independent and empty for v1', () => {
  assert.equal(forgeCommandIsRouted('deal.set_stage_closed'), false)
  assert.equal(forgeCommandIsRouted('forge.story.hold'), false, 'no forge.* commands yet in v1')
})
