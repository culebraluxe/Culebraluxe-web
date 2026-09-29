import assert from 'node:assert/strict'
import test from 'node:test'

import { WorkflowEngine } from '@/workflow_engine/lib/workflow/engine'
import { FakeSql } from '@/testv2/engine_tests/fake-sql'
import { stubEvaluator } from '@/testv2/engine_tests/fixtures'
import { parseForgeSdlc } from '@/legacy/workflow_app/definitions/forge-sdlc'
import { projectForgeGateFacts, type ForgeGateEvidence } from '@/legacy/workflow_app/forge/forge-facts'

// ---------------------------------------------------------------------------
// THE ENGINE ACTUALLY ROUTES ASSAY (work package A).
//
// The v6 XML got an ASSAY branch and a source-string test said so — but a source-string test cannot tell you
// what the ENGINE does with the definition it loads. This drives the REAL v6 definition through the REAL
// WorkflowEngine and reads the node the engine chooses.
//
// The point is not "the XML contains the word ASSAY". It is that a run whose facts say ASSAY lands on the
// deterministic verification node, and that NO Smith task is created on the way.
// ---------------------------------------------------------------------------

const A = 'a'.repeat(40)

async function driveUntilFirstRealNode(initial: ForgeGateEvidence, decision: 'ASSAY' | 'SMITH') {
  const fake = new FakeSql()
  fake.seedDefinition('FORGE_SDLC', 1, parseForgeSdlc().graph)
  let evidence: ForgeGateEvidence = { ...initial }
  const visited: string[] = []
  let firstNodeAfterLead: string | null = null
  let leadDecided = false
  const app = {
    async readFacts() {
      return projectForgeGateFacts(evidence)
    },
    async executeCommand(request: { commandId: string; commandType: string }) {
      return { commandId: request.commandId, outcome: 'success' as const }
    },
  }
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app })
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'FORGE_SDLC',
    version: 1,
    startedBy: 'test',
    variables: projectForgeGateFacts(evidence),
    subject: { subjectType: 'story', subjectId: `STORY-${decision}` },
  })

  for (let guard = 0; guard < 40; guard++) {
    const instance = fake.store.processInstances.find((row) => row.id === processInstanceId)!
    if (instance.status !== 'active') break
    const ready = fake.store.tasks.filter(
      (task) => task.process_instance_id === processInstanceId && task.status === 'ready',
    )
    if (ready.length === 0) break
    const nodeIds = ready.map((task) => String(fake.store.tokens.find((t) => t.id === task.token_id)!.node_id))
    // THE ROUTED NODE IS THE FIRST ONE READY *AFTER THE LEAD DECIDED*. Earlier phases (scout, architect) run
    // before the Lead, so stopping at the first non-Lead node would read the phase BEFORE the decision and call
    // it the routing — which is exactly what a naive version of this test did.
    if (leadDecided) {
      firstNodeAfterLead = nodeIds[0] ?? null
      visited.push(...nodeIds)
      break
    }
    visited.push(...nodeIds)
    for (const task of ready) {
      const node = String(fake.store.tokens.find((t) => t.id === task.token_id)!.node_id)
      const publicTask = await engine.getTask(task.id)
      const worker = publicTask?.candidates[0] ?? 'worker'
      await engine.claimTask(task.id, worker)
      // The Lead records its decision the way it does in production: into the facts the definition reads.
      const formData = node === 'lead_pre' ? { leadDecision: decision, candidateSha: A } : {}
      evidence = { ...evidence, ...formData }
      await engine.completeTask({ taskId: task.id, userId: worker, transitionName: 'complete', formData })
      if (node === 'lead_pre') leadDecided = true
    }
  }

  return { firstNodeAfterLead, visited, fake, processInstanceId }
}

test('engine: an ASSAY fact routes the real v6 definition to the deterministic verification node', async () => {
  const result = await driveUntilFirstRealNode(
    { workType: 'FEATURE', scoutRequired: false, leadDecision: 'ASSAY', candidateSha: A },
    'ASSAY',
  )
  assert.equal(result.firstNodeAfterLead, 'qa_verify', `visited: ${result.visited.join(' -> ')}`)
})

test('engine: NO Smith task is created on an ASSAY route', async () => {
  const result = await driveUntilFirstRealNode(
    { workType: 'FEATURE', scoutRequired: false, leadDecision: 'ASSAY', candidateSha: A },
    'ASSAY',
  )
  const smithTasks = result.fake.store.tasks.filter(
    (task) => task.process_instance_id === result.processInstanceId && /Smith/i.test(String(task.name)),
  )
  assert.equal(smithTasks.length, 0, `Smith tasks: ${smithTasks.map((t) => t.name).join(', ')}`)
  assert.equal(result.visited.includes('smith'), false, result.visited.join(' -> '))
  assert.equal(result.visited.includes('smith_split_work'), false, result.visited.join(' -> '))
})

test('engine: a SMITH fact still routes to the Smith node — the ASSAY branch disturbed nothing', async () => {
  const result = await driveUntilFirstRealNode({ workType: 'FEATURE', scoutRequired: false }, 'SMITH')
  assert.equal(result.firstNodeAfterLead, 'smith', `visited: ${result.visited.join(' -> ')}`)
})
