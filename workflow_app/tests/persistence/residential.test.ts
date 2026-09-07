import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  assertEngineSchema,
  PersistenceFixture,
} from '../../../testv2/engine_tests/persistence/harness'
import { parseReSupermodel } from '../../definitions/re-supermodel'

// Real residential XML (RE_supermodel-v1.xml) driven against the DEV database:
// the 10-way fork_tracks join (title/tax/funds/documents + six conditional
// tracks that skip when facts are absent) must release exactly once when the
// four required human tasks complete concurrently.
test('CRM-14B: RE_supermodel 10-way fork releases exactly once under concurrent completion (real Postgres)', async () => {
  await assertEngineSchema()
  const parsed = parseReSupermodel()
  const f = new PersistenceFixture()
  try {
    await f.seedDefinition(parsed.key, parsed.version, parsed.graph)
    // CRM-21: the closing_documents_gate consumes the derived
    // closingDocumentsReady fact; this scenario simulates a complete signed
    // closing packet so the joined flow proceeds to the closing task.
    const engine = f.makeEngine({
      app: { readFacts: async () => ({ closingDocumentsReady: true }) },
    })

    const { processInstanceId } = await engine.startProcess({
      definitionKey: parsed.key,
      version: parsed.version,
      tenantId: f.tenantId,
      startedBy: 'tester',
      variables: {},
      // The engine only refreshes facts for a subject-backed instance
      // (ApplicationPort.readFacts). Production always starts the RE workflow
      // with the deal subject (see workflow_app/runtime.ts), so the test must
      // set one for the closing_documents_gate to see closingDocumentsReady.
      subject: { subjectType: 'deal', subjectId: 'deal-residential-test' },
    })

    const byName = async (name: string) => {
      const tasks = (await f.tasks(processInstanceId)) as Array<{ id: string; name: string }>
      const t = tasks.find((x) => x.name === name)
      assert.ok(t, 'expected task ' + name)
      return t!
    }

    // Contract / P&S preparation + execution, then the under-contract command.
    await engine.completeTask({
      taskId: (await byName('Contract / P&S Preparation')).id,
      userId: 'broker',
      transitionName: 'prepared',
    })
    await engine.completeTask({
      taskId: (await byName('Contract / P&S Executed')).id,
      userId: 'broker',
      transitionName: 'executed',
    })

    // The four required tracks now await human completion; the six conditional
    // tracks routed straight to the join (no applicable facts).
    const required = ['Title / Legal', 'Tax / Municipal Clearance', 'Funds Ready', 'Closing Documents']
    const tasks = await f.tasks(processInstanceId)
    const requiredTasks = required.map((name) => {
      const t = tasks.find((x: any) => x.name === name)
      assert.ok(t, 'expected required task ' + name)
      return t as { id: string }
    })

    await Promise.all(
      requiredTasks.map((t) =>
        engine.completeTask({ taskId: t.id, userId: 'sme', transitionName: 'done' }),
      ),
    )

    const joined = await f.events(processInstanceId, 'token.joined')
    assert.equal(joined.length, 1, 'exactly one token.joined event (residential fork)')

    const joinedRow = joined[0] as any
    assert.equal(joinedRow.data?.joinNodeId, 'join_tracks', 'the residential join released')

    // ready_to_close is a state node (auto-advances in this engine); the
    // joined flow continues to the closing schedule and stops at the single
    // closing confirmation task.
    const tokens = (await f.tokens(processInstanceId)) as Array<Record<string, any>>
    const active = tokens.filter((t) => t.status === 'active')
    assert.equal(active.length, 1, 'exactly one active token after the residential join')
    assert.equal(active[0].node_id, 'closing')

    const inst = await f.instance(processInstanceId)
    assert.equal(inst.status, 'active', 'process remains active after join (pre-closing)')
  } finally {
    await f.cleanup()
  }
})
