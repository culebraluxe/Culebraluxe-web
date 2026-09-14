import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import {
  DEFAULT_MAX_GENERATION_TURNS,
  GENERATION_TURN_CAP_ENV,
  assessGenerationTurnBudget,
  resolveGenerationTurnCap,
} from '../forge/model-turn-budget'

// ---------------------------------------------------------------------------
// MAP (arXiv 2512.04123): production agents are short, structured and boxed. 68% of
// surveyed practitioners cap at ten model steps, about half at five, ~80% run a predefined
// workflow. Our own healthy FEATURE generation costs five turns, so the cap is one integer
// per generation and it fails CLOSED.
//
// The failure this prevents is not slowness: it is a generation that keeps looking
// productive, one turn at a time, and is read as "still working" instead of "looping".
// ---------------------------------------------------------------------------

/** An env with only the cap set, typed honestly. */
const envWith = (value?: string): NodeJS.ProcessEnv => {
  const env: NodeJS.ProcessEnv = { NODE_ENV: 'test' }
  if (value !== undefined) env[GENERATION_TURN_CAP_ENV] = value
  return env
}

test('MAP cap: the default is the most common production ceiling', () => {
  assert.equal(resolveGenerationTurnCap(envWith()), DEFAULT_MAX_GENERATION_TURNS)
  assert.equal(DEFAULT_MAX_GENERATION_TURNS, 10, 'MAP: 68% cap at <=10 model steps')
})

test('MAP cap: an operator may raise or lower it', () => {
  assert.equal(resolveGenerationTurnCap(envWith('4')), 4)
  assert.equal(resolveGenerationTurnCap(envWith('25')), 25)
})

test('MAP cap: a broken value never means UNLIMITED', () => {
  // The dangerous direction is silent unboundedness, so a garbage value falls back to the
  // default and an absurd one is clamped — never widened, never infinite.
  assert.equal(resolveGenerationTurnCap(envWith('')), DEFAULT_MAX_GENERATION_TURNS)
  assert.equal(resolveGenerationTurnCap(envWith('lots')), DEFAULT_MAX_GENERATION_TURNS)
  assert.equal(resolveGenerationTurnCap(envWith('0')), 1, 'clamped up to the minimum')
  assert.equal(resolveGenerationTurnCap(envWith('-5')), 1, 'a negative cap is still a cap')
  assert.equal(resolveGenerationTurnCap(envWith('9999')), 100, 'clamped down, not honoured')
})

test('MAP cap: a healthy FEATURE generation has room, and stays allowed', () => {
  // Measured: architect, lead_pre, smith, post, qa = 5 turns.
  for (const turnsUsed of [0, 1, 4, 5, 9]) {
    const verdict = assessGenerationTurnBudget({ turnsUsed })
    assert.equal(verdict.allowed, true, `${turnsUsed} turns must be allowed under the default`)
  }
})

test('MAP cap: AT the cap the next turn does not happen, and the reason is actionable', () => {
  const verdict = assessGenerationTurnBudget({ turnsUsed: 10 })
  assert.equal(verdict.allowed, false)
  if (verdict.allowed) return
  assert.equal(verdict.code, 'GENERATION_TURN_CAP')
  assert.equal(verdict.turnsUsed, 10)
  assert.equal(verdict.cap, 10)
  // The message must say what happened, what to do, and how to authorise a longer run.
  assert.match(verdict.reason, /already dispatched 10 turns \(cap 10\)/)
  assert.match(verdict.reason, /ENGINE QUEUE/)
  assert.match(verdict.reason, new RegExp(GENERATION_TURN_CAP_ENV))
})

test('MAP cap: an explicit cap is honoured exactly (boundary is >=, not >)', () => {
  assert.equal(assessGenerationTurnBudget({ turnsUsed: 3, cap: 4 }).allowed, true)
  assert.equal(assessGenerationTurnBudget({ turnsUsed: 4, cap: 4 }).allowed, false)
  assert.equal(assessGenerationTurnBudget({ turnsUsed: 5, cap: 4 }).allowed, false)
})

test('MAP cap: a nonsense count cannot buy extra turns', () => {
  // A failed count must not fail OPEN. Non-finite reads as 0 (the generation is young), and
  // a negative read cannot be used to argue for a bigger budget.
  assert.equal(assessGenerationTurnBudget({ turnsUsed: Number.NaN }).allowed, true)
  assert.equal(assessGenerationTurnBudget({ turnsUsed: -20 }).turnsUsed, 0)
})

// --- the wiring fences -------------------------------------------------------
//
// The rule is only real if it runs BEFORE a turn is dispatched. These read the runner
// source, the same way the findings-scope and recovery-CAS fences do, because the behaviour
// needs a live control plane.

const RUNNER = readFileSync(new URL('../forge/agent-runtime-role-runner.ts', import.meta.url), 'utf8')

test('MAP cap: the runner consults the budget before dispatching a turn', () => {
  const budgetAt = RUNNER.indexOf('assessGenerationTurnBudget({')
  const dispatchAt = RUNNER.indexOf('await work.enqueue(')

  assert.ok(budgetAt > 0, 'the runner must assess the turn budget')
  assert.ok(dispatchAt > 0, 'the runner must dispatch turns (or this fence is stale)')
  assert.ok(
    budgetAt < dispatchAt,
    'the budget must be checked BEFORE the turn is enqueued — a door after the turn is a hope',
  )
  // The cap now also names the FIRST VIOLATION it noticed (AgentRx) before throwing.
  assert.match(RUNNER, /if \(!turnBudget\.allowed\) \{/)
  assert.match(RUNNER, /throw new Error\(`\$\{turnBudget\.reason\} \$\{line\}`\)/)
  assert.match(
    RUNNER,
    /turnsUsed: await countForgeGenerationTurns\(String\(task\.processInstanceId\)\)/,
    'the count must come from the engine ledger for THIS generation',
  )
})

test('MAP cap: the ledger really counts, and cannot silently report zero rows', () => {
  const ledger = readFileSync(
    new URL('../../db/forge-engine-task-execution.ts', import.meta.url),
    'utf8',
  )
  const counter = ledger.slice(ledger.indexOf('export async function countForgeGenerationTurns'))
  assert.match(counter, /select count\(\*\)::int as turns/, 'the count must be the database count')
  assert.match(counter, /where process_instance_id = \$\{processInstanceId\}/, 'scoped to the generation')
})
