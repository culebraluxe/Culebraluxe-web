import assert from 'node:assert/strict'
import test from 'node:test'

import { WorkflowEngine } from '../../workflow_engine/lib/workflow/engine'
import { FakeSql } from '../../testv2/engine_tests/fake-sql'
import { stubEvaluator } from '../../testv2/engine_tests/fixtures'
import { parseForgeSdlc } from '../definitions/forge-sdlc'
import { projectForgeGateFacts, type ForgeGateEvidence } from '../forge/forge-facts'

const A = 'a'.repeat(40)
const B = 'b'.repeat(40)
const C = 'c'.repeat(40)

type Scenario = {
  initial: ForgeGateEvidence
  task?: (nodeId: string, visit: number, evidence: ForgeGateEvidence) => ForgeGateEvidence
  command?: (commandType: string, visit: number, evidence: ForgeGateEvidence) => ForgeGateEvidence
}

async function runScenario(scenario: Scenario) {
  const fake = new FakeSql()
  fake.seedDefinition('FORGE_SDLC', 1, parseForgeSdlc().graph)
  let evidence: ForgeGateEvidence = { ...scenario.initial }
  const taskVisits = new Map<string, number>()
  const commandVisits = new Map<string, number>()
  const steps: string[] = []
  const app = {
    async readFacts() {
      return projectForgeGateFacts(evidence)
    },
    async executeCommand(request: { commandId: string; commandType: string }) {
      const visit = (commandVisits.get(request.commandType) ?? 0) + 1
      commandVisits.set(request.commandType, visit)
      evidence = {
        ...evidence,
        ...(scenario.command?.(request.commandType, visit, evidence) ?? {}),
      }
      steps.push(request.commandType)
      return { commandId: request.commandId, outcome: 'success' as const }
    },
  }
  const engine = new WorkflowEngine(fake.sql, { evaluate: stubEvaluator, app })
  const { processInstanceId } = await engine.startProcess({
    definitionKey: 'FORGE_SDLC',
    version: 1,
    startedBy: 'test',
    variables: projectForgeGateFacts(evidence),
    subject: { subjectType: 'story', subjectId: 'STORY-1' },
  })

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
      if (nodeId === 'hold' || nodeId === 'repair_requirements') {
        return { fake, evidence, steps, processInstanceId, humanNode: nodeId }
      }
      const visit = (taskVisits.get(nodeId) ?? 0) + 1
      taskVisits.set(nodeId, visit)
      const observed = scenario.task?.(nodeId, visit, evidence) ?? defaultTaskEvidence(nodeId, evidence)
      evidence = { ...evidence, ...observed }
      steps.push(nodeId)
      const publicTask = await engine.getTask(task.id)
      const worker = publicTask?.candidates[0] ?? 'worker'
      await engine.claimTask(task.id, worker)
      await engine.completeTask({
        taskId: task.id,
        userId: worker,
        transitionName: 'complete',
        formData: observed,
      })
    }
  }
  return { fake, evidence, steps, processInstanceId, humanNode: null }
}

function defaultTaskEvidence(nodeId: string, evidence: ForgeGateEvidence): ForgeGateEvidence {
  switch (nodeId) {
    case 'lead_pre':
      return { leadDecision: 'SMITH' }
    case 'smith':
    case 'repair_smith':
    case 'smith_split_work':
      return { candidateSha: A }
    case 'lead_post':
      return { candidateSha: evidence.candidateSha ?? A, qaReviewRequired: false }
    case 'qa_verify':
      return { qaPassed: true, qaVerifiedSha: evidence.candidateSha }
    case 'production_smoke':
      return {
        productionVerified: true,
        productionVerifiedSha: evidence.deploymentRequired
          ? evidence.deployedSha
          : evidence.publishedSha,
      }
    default:
      return {}
  }
}

function defaultCommand(commandType: string, _visit: number, evidence: ForgeGateEvidence) {
  switch (commandType) {
    case 'forge.publish_candidate':
      return {
        publishSucceeded: true,
        publishedSha: evidence.candidateSha,
        migrationRequired: evidence.migrationRequired ?? false,
        derivedRefreshRequired: evidence.derivedRefreshRequired ?? false,
        deploymentRequired: evidence.deploymentRequired ?? false,
      }
    case 'forge.verify_dev_migration':
      return { devMigrationVerified: true }
    case 'forge.verify_prod_migration':
      return { prodMigrationVerified: true }
    case 'forge.verify_derived_models':
      return { derivedRefreshVerified: true }
    default:
      return {}
  }
}

test('ENG-FORGE-V10: FEATURE SMITH path completes with exact release lineage', async () => {
  const result = await runScenario({
    initial: { workType: 'FEATURE', scoutRequired: false },
    command: defaultCommand,
    task: (node, _visit, evidence) =>
      node === 'lead_post' ? { candidateSha: B, qaReviewRequired: false } : defaultTaskEvidence(node, evidence),
  })
  const instance = result.fake.store.processInstances.find((row) => row.id === result.processInstanceId)!
  assert.equal(instance.outcome, 'completed')
  assert.equal(result.evidence.candidateSha, B)
  assert.equal(result.evidence.qaVerifiedSha, B)
  assert.equal(result.evidence.publishedSha, B)
  assert.equal(result.evidence.productionVerifiedSha, B)
})

test('ENG-FORGE-V10: BUG/HOTFIX/RESEARCH entry routing remains XML-owned', async () => {
  const bug = await runScenario({
    initial: { workType: 'BUG', rootCauseKnown: false },
    command: defaultCommand,
    task: (node, _visit, evidence) =>
      node === 'diagnose_scout' ? { rootCauseKnown: true } : defaultTaskEvidence(node, evidence),
  })
  assert.ok(bug.steps.includes('diagnose_scout'))

  const hotfix = await runScenario({
    initial: { workType: 'HOTFIX', architectureSuspect: false },
    command: defaultCommand,
  })
  assert.ok(!hotfix.steps.includes('architect'))
  assert.ok(hotfix.steps.includes('lead_pre'))

  const research = await runScenario({
    initial: { workType: 'RESEARCH' },
    command: defaultCommand,
    task: (node, _visit, evidence) =>
      node === 'research_architect'
        ? { researchDisposition: 'ARCHIVE' }
        : defaultTaskEvidence(node, evidence),
  })
  assert.ok(research.steps.includes('research_scout'))
  assert.ok(!research.steps.includes('smith'))
})

test('ENG-FORGE-V10: QA failure repairs Smith and verifies the new integrated candidate', async () => {
  const result = await runScenario({
    initial: { workType: 'FEATURE', scoutRequired: false },
    command: defaultCommand,
    task: (node, visit, evidence) => {
      if (node === 'qa_verify' && visit === 1) return { qaPassed: false, failureClass: 'CODE_DEFECT' }
      if (node === 'repair_smith') return { candidateSha: C }
      return defaultTaskEvidence(node, evidence)
    },
  })
  assert.equal(result.steps.filter((step) => step === 'qa_verify').length, 2)
  assert.ok(result.steps.includes('repair_smith'))
  assert.equal(result.evidence.candidateSha, C)
  assert.equal(result.evidence.qaVerifiedSha, C)
})

test('ENG-FORGE-V10: publish repair revisits the command with a new command identity', async () => {
  const result = await runScenario({
    initial: { workType: 'FEATURE', scoutRequired: false },
    command: (type, visit, evidence) => {
      if (type === 'forge.publish_candidate' && visit === 1) {
        return {
          publishSucceeded: false,
          failureClass: 'PUBLISH_CONFLICT',
          failedReleaseStage: 'PUBLISH',
        }
      }
      return defaultCommand(type, visit, evidence)
    },
  })
  const commands = result.fake.store.processCommands.filter(
    (row) => row.node_id === 'publish_candidate',
  )
  assert.equal(commands.length, 2)
  assert.deepEqual(commands.map((row) => row.visit_sequence), [1, 2])
  assert.notEqual(commands[0].command_id, commands[1].command_id)
  assert.ok(result.steps.includes('repair_devops'))
})

test('ENG-FORGE-V10: migration and derived refresh execute and verify in order', async () => {
  const result = await runScenario({
    initial: {
      workType: 'MIGRATION',
      migrationRequired: true,
      derivedRefreshRequired: true,
      deploymentRequired: false,
    },
    command: defaultCommand,
  })
  const expected = [
    'forge.publish_candidate',
    'forge.migrate_dev',
    'forge.verify_dev_migration',
    'forge.migrate_prod',
    'forge.verify_prod_migration',
    'forge.refresh_derived_models',
    'forge.verify_derived_models',
  ]
  assert.deepEqual(result.steps.filter((step) => step.startsWith('forge.')), expected)
  assert.equal(result.evidence.devMigrationVerified, true)
  assert.equal(result.evidence.prodMigrationVerified, true)
  assert.equal(result.evidence.derivedRefreshVerified, true)
})

test('ENG-FORGE-V10: SPLIT creates exactly three Smith tasks and joins once', async () => {
  const result = await runScenario({
    initial: { workType: 'FEATURE', scoutRequired: false, splitCount: 3 },
    command: defaultCommand,
    task: (node, _visit, evidence) =>
      node === 'lead_pre'
        ? { leadDecision: 'SPLIT', splitCount: 3 }
        : defaultTaskEvidence(node, evidence),
  })
  assert.equal(result.steps.filter((step) => step === 'smith_split_work').length, 3)
  assert.equal(
    result.steps.filter((step) => step === 'lead_post').length,
    1,
    result.steps.join(' -> '),
  )
  const splitTasks = result.fake.store.tasks.filter((task) => task.name === 'Smith Split')
  assert.deepEqual(
    splitTasks.map((task) => task.form_data.splitBranchIndex),
    [0, 1, 2],
  )
})

test('ENG-FORGE-V10: HOLD remains a durable human gate and never completes', async () => {
  const result = await runScenario({
    initial: { workType: 'FEATURE', scoutRequired: false },
    command: defaultCommand,
    task: (node, _visit, evidence) =>
      node === 'lead_pre' ? { leadDecision: 'HOLD' } : defaultTaskEvidence(node, evidence),
  })
  assert.equal(result.humanNode, 'hold')
  const instance = result.fake.store.processInstances.find((row) => row.id === result.processInstanceId)!
  assert.equal(instance.status, 'active')
})
