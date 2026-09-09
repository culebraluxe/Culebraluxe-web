import { buildLaneEnqueue } from '../../agent-runtime/enqueue-lane'
import { buildGroundingDirective, buildRunGuardrailsDirective, buildRunPassDirective, buildRtkCompressionDirective } from '../../agent-runtime/run-guardrails'
import {
  createAgentRuntimeRegistry,
  defaultDeepSeekConfig,
  parseBuilderFlashOverride,
} from '../../agent-runtime/factory'
import {
  buildAgentInvokerWorkspaces,
  executeClaimedAgentCommand,
  resolveForgeExecutionRunId,
} from '../../agent-runtime/invoker'
import {
  buildRepoContextQuery,
  latestScoutResearch,
  runRepoContextTaskPacket,
  withRepoContextPacket,
  withScoutResearch,
} from '../../agent-runtime/repo-context'
import {
  SqlAgentRunRepository,
  SqlAgentWorkRepository,
} from '../../agent-runtime/repositories'
import { buildForgeSmokeFlashTeam } from '../../agent-runtime/smoke-team'
import { getAgentWorkItem } from '../../db/agent-work'
import {
  finishForgeEngineTaskExecution,
  linkForgeEngineTaskExecution,
} from '../../db/forge-engine-task-execution'
import { getForgeLeadRunRecord } from '../../db/forge-run'
import { readForgeRepairLedger } from '../../db/forge-repair-ledger'
import { readForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { getStoryboardStory, setStoryArchitectBrief, setStoryScoutPacket } from '../../db/storyboard'
import { parseExecutionEnvironment } from '../../lib/execution-target'
import { interactiveSql } from '../../lib/neon-interactive'
import type { ForgeRoleRunner } from './forge-executor'
import {
  buildSelfHealDirective,
  deliverableEnforcementEnabled,
  parseDeliverableRepromptBudget,
  rawRoleOutput,
} from './agents/forge-phase-agent'
import { forgeAgentFor } from './agents/role-agents'
import {
  forgeEvidenceFromAgentResult,
  forgeRoleNodePlan,
} from './forge-role-mapping'
import {
  readLegacyMarkerGateEvidence,
  readTypedGateEvidence,
} from './forge-typed-evidence'
import type { ForgeGateEvidence } from './forge-facts'

export type AgentRuntimeForgeRunnerOptions = {
  workerId: string
  executionEnvironment?: string | null
}

const SCOUT_RESEARCH_CONSUMERS = new Set(['architect', 'lead', 'smith', 'inspector'])

function runtimeInterrupted(resultStatus: string, completion: number): boolean {
  return completion < 100 || /interrupted|error|cancelled/i.test(resultStatus)
}

export function createAgentRuntimeForgeRoleRunner(
  options: AgentRuntimeForgeRunnerOptions,
): ForgeRoleRunner {
  const work = new SqlAgentWorkRepository(async () => interactiveSql as never)
  const runs = new SqlAgentRunRepository(async () => interactiveSql as never)
  const registry = createAgentRuntimeRegistry({
    team: buildForgeSmokeFlashTeam(),
    deepseek: defaultDeepSeekConfig(),
    builderFlashOverride: parseBuilderFlashOverride(
      process.env.FORGE_PROVIDER_BUILDER_FLASH ?? null,
    ),
  })

  return async (nodeId, task) => {
    const subjectRows = await interactiveSql`
      select subject_id
      from process_instances
      where id = ${task.processInstanceId}
      limit 1
    `
    const resolvedStory = await getStoryboardStory(String(subjectRows[0]?.subject_id ?? ''))
    if (!resolvedStory) throw new Error(`Forge engine task ${task.taskId} has no Storyboard story`)

    // Bounded self-heal. When the enforced gate (default ON; disable with
    // FORGE_ENFORCE_DELIVERABLES=0) HOLDs a *successful* run for a missing
    // deliverable / routing decision, we
    // re-run the role up to `totalAttempts` times (1 initial + the
    // FORGE_DELIVERABLE_RETRIES budget), feeding each corrective re-run a
    // directive that names exactly what was missing. Only a fixable miss on an
    // otherwise-successful run triggers a reprompt; hard runtime failures below
    // still throw immediately. On the final attempt a miss is a real HOLD.
    const enforceDeliverables = deliverableEnforcementEnabled(
      process.env.FORGE_ENFORCE_DELIVERABLES,
    )
    const totalAttempts = enforceDeliverables
      ? Math.max(1, parseDeliverableRepromptBudget(process.env.FORGE_DELIVERABLE_RETRIES) + 1)
      : 1
    let correctiveNote = ''
    for (let attempt = 0; attempt < totalAttempts; attempt++) {
    const plan = forgeRoleNodePlan(nodeId)
    const branchInstruction =
      nodeId === 'smith_split_work'
        ? `Split branch ${String(task.formData.splitBranchIndex ?? '?')} of ${String(task.formData.splitBranchCount ?? '?')}. Bounded branch contract: ${JSON.stringify(task.formData.splitBranch ?? null)}`
        : null
    const identityInstruction =
      `Forge engine task=${task.taskId}; process=${task.processInstanceId}; node=${nodeId}. ` +
      'Execute this responsibility only. The XML engine owns all next-step routing.'

    const repoContextInstruction =
      plan.lane === 'scout'
        ? withRepoContextPacket(
            null,
            runRepoContextTaskPacket({
              workspace: process.cwd(),
              task: buildRepoContextQuery(resolvedStory),
            }),
          )
        : null

    const priorScoutInstruction = SCOUT_RESEARCH_CONSUMERS.has(plan.lane)
      ? withScoutResearch(
          null,
          latestScoutResearch(await runs.listForStory(resolvedStory.id)),
        )
      : null

    const extraInstructions = [
      correctiveNote,
      identityInstruction,
      branchInstruction,
      plan.evidenceInstruction,
      repoContextInstruction,
      priorScoutInstruction,
      buildRunGuardrailsDirective(),
      buildRunPassDirective(),
      buildRtkCompressionDirective(),
      plan.lane === 'architect' || plan.lane === 'lead' ? buildGroundingDirective() : null,
    ]
      .filter(Boolean)
      .join('\n\n')
    const lane = buildLaneEnqueue({
      lane: plan.lane,
      story: resolvedStory,
      registry,
      leadPhase: plan.leadPhase,
      extraInstructions,
    })
    if (!lane.ok) {
      throw new Error(`Forge ${nodeId} execution contract rejected: ${lane.reason}`)
    }
    if (!lane.envelope) throw new Error(`Forge ${nodeId} produced no execution envelope`)

    const target = parseExecutionEnvironment(
      options.executionEnvironment ?? process.env.EXECUTION_ENV,
      'DEV',
    )
    const queued = await work.enqueue({
      storyId: resolvedStory.id,
      ...lane.envelope,
      executionEnvironment: target,
      executionPolicy: 'Unattended OK',
    })
    if (attempt === 0) {
      await linkForgeEngineTaskExecution({
        taskId: task.taskId,
        processInstanceId: task.processInstanceId,
        tokenId: task.tokenId,
        storyId: resolvedStory.id,
        nodeId,
        workItemId: queued.id,
        workerId: options.workerId,
      })
    }
    const claimed = await work.claimSpecific(queued.id, options.workerId)
    if (!claimed) {
      throw new Error(`Forge engine task ${task.taskId} could not claim agent work item ${queued.id}`)
    }

    const durableClaim = await work.get(queued.id)
    if (!durableClaim) {
      throw new Error(
        `Forge engine task ${task.taskId} claimed agent work item ${queued.id}, but the durable row could not be reloaded`,
      )
    }

    // WORKSPACE-01: canonical Git lineage = processInstanceId + execution
    // generation (durable replan attempts). Resolved at EXECUTION time from the
    // engine task — never frozen from options.workerId at runner build time.
    // All serial roles reuse this generation key; only smith_split_work fans
    // out a bounded child workspace (splitBranchIndex), and a REPLAN advances
    // the generation, giving the next generation a clean workspace.
    const ledger = await readForgeRepairLedger(resolvedStory.id, interactiveSql as never)
    const replanAttempts = ledger?.replanAttempts ?? 0
    const splitChild =
      nodeId === 'smith_split_work'
        ? String(task.formData.splitBranchIndex ?? task.formData.splitIndex ?? 0)
        : null
    const executionId = resolveForgeExecutionRunId(task.processInstanceId, replanAttempts, splitChild)
    const workspaces = buildAgentInvokerWorkspaces(options.workerId, undefined, executionId)

    let result
    try {
      result = await executeClaimedAgentCommand(
        options.workerId,
        { workItem: durableClaim, story: resolvedStory },
        {
          work,
          runs,
          registry,
          ...(workspaces ? { workspaces } : {}),
          enforceExecutionContract: true,
        },
      )
    } catch (error) {
      await finishForgeEngineTaskExecution(task.taskId, {
        status: 'interrupted',
        error: String((error as Error)?.message ?? error),
      })
      throw error
    }

    if (runtimeInterrupted(result.evidence.resultStatus, result.evidence.completion)) {
      const reason =
        `Forge ${nodeId} runtime did not complete cleanly: ` +
        `${result.evidence.resultStatus} (${result.evidence.completion}%)`
      await finishForgeEngineTaskExecution(task.taskId, {
        status: 'interrupted',
        error: reason,
      })
      throw new Error(reason)
    }

    const current = await readForgeWorkflowEvidence(resolvedStory.id)
    const finishedItem = await getAgentWorkItem(result.workItemId)
    const leadDecision = finishedItem?.storyRunId
      ? await getForgeLeadRunRecord(finishedItem.storyRunId)
      : null

    const typed = readTypedGateEvidence(result.evidence)
    const mapped = forgeEvidenceFromAgentResult({
      nodeId,
      result: result.evidence,
      current,
      leadDecision,
    })
    const marked = readLegacyMarkerGateEvidence(result.evidence)
    const evidence: ForgeGateEvidence = { ...mapped, ...marked, ...(typed ?? {}) }

    // ENG-FORGE-PHASE-AGENT: the marshaling/deliverable tail is owned by a
    // ForgePhaseAgent for this node (findings parse for Scout/Architect, Lead
    // PRE shape override, Scout packet). This replaces the scattered
    // isArchitectNode/isScoutNode/lead_pre conditionals with a role contract.
    const agent = forgeAgentFor(nodeId)
    const raw = rawRoleOutput(result.evidence.notes, result.evidence.testsSummary)
    agent.marshalFindings(evidence, raw)
    agent.applyLeadShape(evidence, current.findings)

    // Write-on-exit to Neon from the role's ACTUAL output (never gated on a model
    // self-format marker). Centralized: the agent declares its Story-field
    // deliverable (Scout -> context_refs packet; Architect -> architect_brief
    // plan) and we persist it generically. These are the durable forward handoffs
    // the next lane's gate requires.
    const successful = /pass|success|complete/i.test(result.evidence.resultStatus)
    let scoutDelivered = false
    let architectDelivered = false
    if (successful) {
      const deliverable = agent.storyDeliverable(raw)
      if (deliverable) {
        try {
          if (deliverable.field === 'context_refs') {
            scoutDelivered = await setStoryScoutPacket(resolvedStory.id, deliverable.text)
          } else {
            architectDelivered = await setStoryArchitectBrief(resolvedStory.id, deliverable.text)
          }
        } catch {
          /* a failed deliverable write must not fail the engine task (DB failures captured at gateway) */
        }
      }
    }

    // ENG-FORGE-PHASE-AGENT Phase 3 — enforced deliverable gate. ON by default
    // (disable with FORGE_ENFORCE_DELIVERABLES=0): a role that reports success
    // but did NOT produce its declared deliverable or a valid routing decision is
    // HOLDed (thrown -> forge-executor releases the task) after the bounded
    // self-heal budget, instead of silently advancing.
    const miss: string[] = []
    if (enforceDeliverables && successful) {
      const missing = agent.missingDeliverables(evidence, raw, scoutDelivered, architectDelivered)
      // Routing-decision validation: even a phase whose WORK persisted must leave a
      // usable engine routing decision (research_architect -> research_disposition;
      // lead -> lead_decision). A null/invalid decision is a HOLD, not a pass.
      const routeMiss = agent.routingDecisionMissing(evidence)
      if (routeMiss) missing.push(`routing:${routeMiss}`)
      miss.push(...missing)
    }

    if (miss.length === 0) {
      await finishForgeEngineTaskExecution(task.taskId, {
        storyRunId: finishedItem?.storyRunId ?? null,
        status: successful ? 'completed' : 'failed',
      })
      return {
        transitionName: 'complete',
        evidence,
      }
    }

    // Bounded self-heal: an otherwise-successful run that missed a deliverable or
    // routing decision is re-run with a corrective directive naming what was
    // missing. Only when the reprompt budget is exhausted do we throw the HOLD.
    if (attempt + 1 < totalAttempts) {
      correctiveNote = buildSelfHealDirective(nodeId, miss, plan.evidenceInstruction)
      continue
    }
    throw new Error(
      `Forge ${nodeId} HOLD (after ${totalAttempts} attempt(s)): role did not deliver ${miss.join(', ')}`,
    )
    }
    // Unreachable: the loop always returns (pass) or throws (final HOLD).
    throw new Error(`Forge ${nodeId} exhausted reprompt attempts without a verdict`)
  }
}
