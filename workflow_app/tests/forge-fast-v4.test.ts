import assert from 'node:assert/strict'
import { test } from 'node:test'

import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FakeSql } from '../../testv2/engine_tests/fake-sql'
import { stubEvaluator } from '../../testv2/engine_tests/fixtures'
import { parseForgeSdlcV4 } from '../definitions/forge-sdlc'
import { projectForgeGateFacts, type ForgeGateEvidence } from '../forge/forge-facts'

// Scope C — FAST lane proof against the real engine, on the (inactive) v4 graph.
const A = 'a'.repeat(40)
const B = 'b'.repeat(40)

function fastCandidate(nodeId: string, evidence: ForgeGateEvidence): Partial<ForgeGateEvidence> {
  switch (nodeId) {
    case 'fast_smith':
      return { candidateSha: evidence.candidateSha ?? A }
    case 'fast_repair_smith':
      return { candidateSha: evidence.candidateSha === A ? B : A }
    case 'fast_qa_verify':
      return { qaPassed: true, qaVerifiedSha: evidence.candidateSha }
    default:
      return {}
  }
}

function fastPublish(evidence: ForgeGateEvidence): Partial<ForgeGateEvidence> {
  return { publishSucceeded: true, publishedSha: evidence.candidateSha }
}

async function runFast(opts: {
  vars: Partial<ForgeGateEvidence> & { workType: 'FAST' }
  task?: (node: string, visit: number, evidence: ForgeGateEvidence) => Partial<ForgeGateEvidence>
  command?: (commandType: string, evidence: ForgeGateEvidence) => Partial<ForgeGateEvidence>
}) {
  const fake = new FakeSql()
  fake.seedDefinition('FORGE_SDLC', 4, parseForgeSdlcV4().graph)
  let evidence: ForgeGateEvidence = { ...opts.vars }
  const taskVisits = new Map<string, number>()
  const steps: string[] = []
  const app = {
    async readFacts() {
      return projectForgeGateFacts(evidence)
    },
    async executeCommand(request: { commandId: string; commandType: string }) {
      const merged = opts.command?.(request.commandType, evidence) ?? fastPublish(evidence)
      evidence = { ...evidence, ...merged }
      steps.push(request.commandType)
      return { commandId: request.commandId, outcome: 'success' as const }
    },
  }
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app })
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'FORGE_SDLC',
    version: 4,
    startedBy: 'test',
    variables: projectForgeGateFacts(evidence),
    subject: { subjectType: 'story', subjectId: 'STORY-FAST' },
  })
  let humanNode: string | null = null
  for (let guard = 0; guard < 100; guard++) {
    const instance = fake.store.processInstances.find((row) => row.id === processInstanceId)!
    if (instance.status !== 'active') break
    const ready = fake.store.tasks.filter(
      (task) => task.process_instance_id === processInstanceId && task.status === 'ready',
    )
    if (ready.length === 0) break
    for (const task of ready) {
      const token = fake.store.tokens.find((row) => row.id === task.token_id)!
      const nodeId = String(token.node_id)
      if (nodeId === 'hold') {
        humanNode = 'hold'
        return { fake, evidence, steps, processInstanceId, humanNode }
      }
      const visit = (taskVisits.get(nodeId) ?? 0) + 1
      taskVisits.set(nodeId, visit)
      const observed = opts.task?.(nodeId, visit, evidence) ?? fastCandidate(nodeId, evidence)
      evidence = { ...evidence, ...observed }
      steps.push(nodeId)
      const publicTask = await engine.getTask(task.id)
      const worker = publicTask?.candidates[0] ?? 'worker'
      await engine.claimTask(task.id, worker)
      await engine.completeTask({ taskId: task.id, userId: worker, transitionName: 'complete', formData: observed })
    }
  }
  const instance = fake.store.processInstances.find((row) => row.id === processInstanceId)
  return {
    fake,
    evidence,
    steps,
    processInstanceId,
    status: instance ? (instance.status as string) : null,
    humanNode,
  }
}

test('Scope C FAST: bounded FAST story drives Smith -> deterministic QA -> publish -> complete', async () => {
  const r = await runFast({ vars: { workType: 'FAST' } })
  assert.equal(r.humanNode, null, r.steps.join(' -> '))
  assert.ok(r.steps.includes('fast_smith'))
  assert.ok(r.steps.includes('fast_qa_verify'))
  assert.ok(r.steps.includes('forge.publish_candidate'))
  // No Architect / Lead / human-confirmation model runs in FAST.
  assert.ok(!r.steps.includes('architect'))
  assert.ok(!r.steps.includes('lead_pre'))
  assert.ok(!r.steps.includes('fast_confirmation'))
})

test('Scope C FAST: a repair cycle stays in FAST and re-runs deterministic QA before publish', async () => {
  let failDone = false
  const r = await runFast({
    vars: { workType: 'FAST' },
    task: (node, _v, evidence) => {
      if (node === 'fast_smith') return { candidateSha: A }
      if (node === 'fast_repair_smith') return { candidateSha: B }
      if (node === 'fast_qa_verify') {
        if (!failDone) {
          failDone = true
          return {
            qaPassed: false,
            qaVerifiedSha: null,
            disposition: 'REPAIR',
            failureClass: 'CODE_DEFECT',
            repairAttempts: 0,
          }
        }
        return { qaPassed: true, qaVerifiedSha: evidence.candidateSha }
      }
      return {}
    },
  })
  assert.equal(r.humanNode, null, r.steps.join(' -> '))
  assert.ok(r.steps.includes('fast_repair_smith'), r.steps.join(' -> '))
  assert.equal(r.steps.filter((s) => s === 'fast_qa_verify').length, 2)
  assert.ok(r.steps.includes('forge.publish_candidate'))
  assert.ok(!r.steps.includes('fast_confirmation'))
})

test('Scope C FAST: a release obligation fails FAST closed to HOLD (no hidden release)', async () => {
  const r = await runFast({ vars: { workType: 'FAST', migrationRequired: true } })
  assert.equal(r.humanNode, 'hold')
  assert.ok(!r.steps.includes('fast_smith'))
  assert.ok(!r.steps.includes('forge.publish_candidate'))
})

test('Scope C FAST: a QA REPLAN-classified failure ends FAST at HOLD rather than auto-replying', async () => {
  const r = await runFast({
    vars: { workType: 'FAST' },
    task: (node, _v, evidence) => {
      if (node === 'fast_smith') return { candidateSha: A }
      if (node === 'fast_qa_verify') {
        return { qaPassed: false, disposition: 'REPLAN', failureClass: 'ARCHITECTURE_GAP', replanAttempts: 0 }
      }
      return {}
    },
  })
  // fast_qa_route: qaRepairEligible false, qaReplanEligible true -> hold.
  assert.equal(r.humanNode, 'hold')
  assert.ok(!r.steps.includes('forge.publish_candidate'))
  assert.ok(!r.steps.includes('fast_repair_smith'))
})
