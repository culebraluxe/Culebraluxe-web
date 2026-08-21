import { test } from 'node:test'
import assert from 'node:assert/strict'

import { withTransaction } from '../../../lib/neon-interactive'
import {
  claimReceipt,
  finalizeReceipt,
  readFinalReceipt,
  replayOutcome,
} from '../../../db/workflow-command-receipt'
import {
  assertEngineSchema,
  ensureCommandEffectTable,
  PersistenceFixture,
} from '../../../workflow_engine/tests/persistence/harness'

// start -> approve (human task) -> do (application command) -> end
const COMMAND_LINE = {
  startNodeId: 'start',
  nodes: {
    start: {
      id: 'start',
      type: 'start',
      transitions: [{ name: 'go', to: 'approve' }],
    },
    approve: {
      id: 'approve',
      type: 'task',
      name: 'Approve',
      transitions: [{ name: 'done', to: 'do' }],
    },
    do: {
      id: 'do',
      type: 'command',
      commandType: 'tunit.do',
      transitions: [{ name: 'go', to: 'end' }],
    },
    end: { id: 'end', type: 'end' },
  },
}

test('ENG-09: command crash/replay boundary — engine rollback after a committed side effect never duplicates it (real Postgres)', async () => {
  await assertEngineSchema()
  await ensureCommandEffectTable()
  const f = new PersistenceFixture()
  const executedCommandIds: string[] = []

  const app = {
    async executeCommand(req: { commandId: string }) {
      executedCommandIds.push(req.commandId)
      return withTransaction(async (tx) => {
        const claimed = await claimReceipt(tx, req.commandId)
        if (claimed) {
          // Business effect persists OUTSIDE the engine transaction.
          await tx`insert into tunit_command_effect (command_id, tenant_id, effect_count) values (${req.commandId}, ${f.tenantId}, 1)`
          await finalizeReceipt(tx, req.commandId, 'success', null, null)
          return { commandId: req.commandId, outcome: 'success' as const, message: null }
        }
        // Deterministic replay: the receipt already exists; never re-apply.
        const receipt = await readFinalReceipt(tx, req.commandId)
        const replay = replayOutcome(receipt)
        return { commandId: req.commandId, outcome: replay.outcome, message: replay.message }
      })
    },
    async readFacts() {
      return {}
    },
  }

  try {
    await f.seedDefinition('tunit_cmd', 1, COMMAND_LINE)

    let armed = true
    const engine = f.makeEngine({
      app,
      hooks: {
        afterCommandSideEffect: async () => {
          if (armed) throw new Error('injected engine failure after command side effect')
        },
      },
    })

    const { processInstanceId } = await engine.startProcess({
      definitionKey: 'tunit_cmd',
      version: 1,
      tenantId: f.tenantId,
      startedBy: 'tester',
    })
    const task = (await f.tasks(processInstanceId))[0] as { id: string }

    // Attempt 1: the app side effect COMMITS, then the engine step fails and
    // rolls back the whole engine transaction.
    await assert.rejects(() =>
      engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' }),
    )

    const effects = await f.rows<{ c: number }>`select count(*)::int as c from tunit_command_effect where tenant_id = ${f.tenantId}`
    assert.equal(effects[0].c, 1, 'side effect committed exactly once')

    const taskAfter = (await f.tasks(processInstanceId))[0] as { status: string }
    assert.equal(taskAfter.status, 'ready', 'engine transaction rolled back (task re-open)')
    const tokens = await f.tokens(processInstanceId)
    assert.equal(tokens[0].node_id, 'approve', 'token rolled back to the task node')
    const cmds = await f.commands(processInstanceId)
    assert.equal(cmds.length, 0, 'no process_commands row survives the rollback')
    const completedEvs = await f.events(processInstanceId, 'command.completed')
    assert.equal(completedEvs.length, 0, 'no command.completed event survives the rollback')

    // Attempt 2: retry the same step. The deterministic commandId replays the
    // committed receipt; the business effect is NOT duplicated.
    armed = false
    await engine.completeTask({ taskId: task.id, userId: 'u', transitionName: 'done' })

    const effects2 = await f.rows<{ c: number }>`select count(*)::int as c from tunit_command_effect`
    assert.equal(effects2[0].c, 1, 'side effect NOT duplicated after retry')
    assert.equal(executedCommandIds.length, 2, 'engine re-invoked the command with the same id')
    assert.equal(executedCommandIds[0], executedCommandIds[1], 'deterministic commandId')

    const cmds2 = await f.commands(processInstanceId)
    assert.equal(cmds2.length, 1, 'process_commands row persisted on retry')
    assert.equal(cmds2[0].command_id, executedCommandIds[0])

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'completed')
  } finally {
    await f.cleanup()
  }
})
