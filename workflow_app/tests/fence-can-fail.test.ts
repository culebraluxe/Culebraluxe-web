import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { after, test } from 'node:test'

import { mergeForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { collectAssayEvidence } from '../forge/agents/assay-collect'
import { runAssay } from '../forge/agents/qa/run'
import { buildAcceptanceMap } from '../forge/agents/qa/types'
import type { AcceptanceCondition, CommandResult } from '../forge/agents/qa/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-FENCE-CAN-FAIL-01 — A FENCE PROVES IT CAN FAIL BEFORE ITS GREEN COUNTS.
//
// ONE fence source, run twice: once with the claimed behaviour (green) and once with it withheld
// (red). The negative control must kill an intended assertion, or the green proves nothing. The
// control creates AND destroys its scratch inside the same command, so no mutant, file or branch
// outlives it. The killing assertion is then carried into the durable evidence writer's columns.
// ---------------------------------------------------------------------------

const ASSERTION = 'the claimed behaviour holds'

/** The SAME fence source for both runs: only CLAIMED_BEHAVIOUR differs. */
const FENCE_SCRIPT =
  "const claimed = process.env.CLAIMED_BEHAVIOUR === '1'; " +
  "console.log(claimed ? '\u2714 the claimed behaviour holds' : '\u2716 the claimed behaviour holds');"

const POSITIVE_COMMAND = `CLAIMED_BEHAVIOUR=1 node -e ${JSON.stringify(FENCE_SCRIPT)}`

/** Scratch the control must destroy inside the same command. */
const SCRATCH = join(tmpdir(), `fence-can-fail-${process.pid}-${Date.now()}`)

/**
 * The negative control: ONE command that creates its scratch, runs the SAME fence with the claimed
 * behaviour withheld, prints that fence's own output, then removes the scratch.
 */
const WRAPPER =
  "const fs=require('fs');const cp=require('child_process');" +
  `const scratch=${JSON.stringify(SCRATCH)};` +
  "fs.mkdirSync(scratch,{recursive:true});fs.writeFileSync(scratch+'/marker','x');" +
  `const r=cp.spawnSync(process.execPath,['-e',${JSON.stringify(FENCE_SCRIPT)}],` +
  "{env:{...process.env,CLAIMED_BEHAVIOUR:'0'},encoding:'utf8'});" +
  "process.stdout.write(r.stdout||'');process.stderr.write(r.stderr||'');" +
  "fs.rmSync(scratch,{recursive:true,force:true});"
const NEGATIVE_COMMAND = `node -e ${JSON.stringify(WRAPPER)}`

const CONDITION: AcceptanceCondition = {
  id: 'claimed-behaviour',
  text: 'the claimed behaviour holds',
  assertions: [ASSERTION],
}

function runCommand(command: string): CommandResult {
  const result = spawnSync('sh', ['-c', command], { encoding: 'utf8' })
  const output = `${result.stdout ?? ''}${result.stderr ?? ''}`
  return {
    command,
    exitCode: result.status ?? 1,
    passed: result.status === 0,
    excerpt: output.replace(/\s+/g, ' ').trim().slice(0, 240),
    output,
    ...(result.status === null ? { unmeasurable: true } : {}),
  }
}

const planWith = (controlCommand: string) => ({
  commands: [POSITIVE_COMMAND],
  conditions: [CONDITION],
  negativeControl: { command: controlCommand },
})

after(() => {
  rmSync(SCRATCH, { recursive: true, force: true })
})

test('the declared fence runs green under the positive control', () => {
  const report = runAssay({ plan: planWith(NEGATIVE_COMMAND), runCommand })
  assert.equal(report.verdict, 'PASS', 'the fence passes and the control killed an intended assertion')
})

test('a negative control that inverts the claimed behaviour fails at least one intended assertion', () => {
  const report = runAssay({ plan: planWith(NEGATIVE_COMMAND), runCommand })
  assert.equal(report.negativeControl?.ran, true, 'the control executed')
  assert.ok(
    report.negativeControl?.killingAssertions.includes(ASSERTION),
    'the intended assertion went red under the inversion',
  )
})

test('the killing assertion is named in the evidence', async () => {
  const report = runAssay({ plan: planWith(NEGATIVE_COMMAND), runCommand })
  assert.deepEqual(report.negativeControl?.killingAssertions, [ASSERTION])
  const calls: Array<{ sql: string; values: unknown[] }> = []
  const fake = (strings: TemplateStringsArray, ...values: unknown[]) => {
    calls.push({ sql: strings.join('?'), values })
    return Promise.resolve([] as unknown[])
  }
  await mergeForgeWorkflowEvidence(
    '00000000-0000-0000-0000-000000000000',
    'FENCE-CAN-FAIL',
    { negativeControl: { ran: true, killingAssertions: [ASSERTION] } },
    fake as never,
  )
  assert.ok(
    calls[0].sql.includes('negative_control_killing_assertion'),
    'the evidence writer has a column for the killing assertion',
  )
  assert.ok(
    calls[0].values.includes(JSON.stringify([ASSERTION])),
    'the killing assertion is the value written',
  )
})

test('the negative control destroys its scratch state in the same command', () => {
  runAssay({ plan: planWith(NEGATIVE_COMMAND), runCommand })
  assert.ok(!existsSync(SCRATCH), 'the control left no scratch behind')
})

test('a fence whose assertions all pass under the negative control is UNPROVEN not PASS', () => {
  // A control that does not invert: the SAME fence passes, so it killed nothing.
  const report = runAssay({ plan: planWith(POSITIVE_COMMAND), runCommand })
  assert.notEqual(report.verdict, 'PASS')
  assert.equal(report.verdict, 'UNPROVEN')
  assert.ok(
    report.blockers.some((blocker) => blocker.startsWith('NEGATIVE_CONTROL_SURVIVED')),
    'the surviving control is named',
  )
  assert.deepEqual(report.negativeControl?.killingAssertions, [])
})

test('the live assay path carries the control outcome and fails closed without a runner', () => {
  const acceptanceMap = buildAcceptanceMap({
    acceptance: [CONDITION.text],
    assertions: { [CONDITION.text]: [ASSERTION] },
  })
  const withRunner = collectAssayEvidence(
    {},
    {
      assayCommands: [POSITIVE_COMMAND],
      acceptanceMap,
      negativeControl: { command: NEGATIVE_COMMAND },
      runCommand,
    },
  )
  assert.equal(withRunner.qaPassed, true, 'the live path passes with a discriminating control')
  assert.deepEqual(withRunner.negativeControl?.killingAssertions, [ASSERTION])

  const noRunner = collectAssayEvidence({}, { assayCommands: [POSITIVE_COMMAND] })
  assert.equal(noRunner.qaPassed, false, 'no runner fails closed, never passes')
})
