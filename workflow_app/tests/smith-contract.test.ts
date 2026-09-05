import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  canLaunch,
  isChangeAllowed,
  validateSmithContract,
  type SmithExecutionContract,
} from '../forge/smith-contract'

// ENG-FORGE-HARDEN-06 — typed Smith node contracts: bounded authority, inputs,
// outputs, evidence obligations; fail before launch when malformed/incomplete.

function c(over: Partial<SmithExecutionContract> = {}): SmithExecutionContract {
  return {
    identity: { storyId: 'S1', nodeId: 'smith_api', attempt: 1, owner: 'w-1' },
    objective: 'implement the API',
    requiredInputs: ['architect_decision'],
    allowedScope: ['src/api'],
    prohibitedScope: ['src/api/migrations'],
    expectedOutputs: ['candidate_api'],
    requiredEvidence: ['pnpm test:api'],
    dependsOn: [],
    ...over,
  }
}

test('a valid contract is launchable', () => {
  assert.equal(validateSmithContract(c()).valid, true)
  const ready = canLaunch(c(), { inputsPresent: ['architect_decision'], predecessorsComplete: [] })
  assert.deepEqual(ready, { launchable: true, errors: [] })
})

test('incomplete/malformed contracts fail validation before launch', () => {
  const noScope = c({ allowedScope: [] })
  const noOutputs = c({ expectedOutputs: [] })
  const noEvidence = c({ requiredEvidence: [] })
  const noObjective = c({ objective: '' })
  assert.equal(validateSmithContract(noScope).valid, false)
  assert.equal(validateSmithContract(noOutputs).valid, false)
  assert.equal(validateSmithContract(noEvidence).valid, false)
  assert.equal(validateSmithContract(noObjective).valid, false)
})

test('missing required input blocks launch', () => {
  const ready = canLaunch(c(), { inputsPresent: [], predecessorsComplete: [] })
  assert.equal(ready.launchable, false)
  assert.ok(ready.errors.some((e) => e.includes('architect_decision')))
})

test('an unsatisfied predecessor dependency blocks launch', () => {
  const contract = c({ dependsOn: ['smith_base'] })
  const ready = canLaunch(contract, { inputsPresent: ['architect_decision'], predecessorsComplete: [] })
  assert.equal(ready.launchable, false)
  assert.ok(ready.errors.some((e) => e.includes('smith_base')))
})

test('prohibited scope always wins, even when the path is also in allowed scope', () => {
  const contract = c({ allowedScope: ['src/api'], prohibitedScope: ['src/api/migrations'] })
  assert.equal(isChangeAllowed(contract, 'src/api/routes.ts'), true)
  assert.equal(isChangeAllowed(contract, 'src/api/migrations/001.sql'), false)
})

test('ownership + attempt identity are carried on the contract', () => {
  const contract = c({ identity: { storyId: 'S1', nodeId: 'smith_ui', attempt: 2, owner: 'w-9' } })
  assert.equal(contract.identity.attempt, 2)
  assert.equal(contract.identity.owner, 'w-9')
  assert.equal(contract.identity.storyId, 'S1')
})
