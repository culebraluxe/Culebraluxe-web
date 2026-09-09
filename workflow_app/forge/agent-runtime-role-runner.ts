import { resolve } from 'node:path'
import { buildLaneEnqueue } from '../../agent-runtime/enqueue-lane'
import { buildGroundingDirective, buildRunGuardrailsDirective, buildRunPassDirective, buildRtkCompressionDirective, buildSmithWorkDecompositionDirective } from '../../agent-runtime/run-guardrails'
import {
  FileContextLessonStore,
  buildContextLessonDirective,
  lessonsForArea,
} from './forge-context-lessons'
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
import { appendForgeRunDetail, getForgeLeadRunRecord } from '../../db/forge-run'
import { readForgeRepairLedger } from '../../db/forge-repair-ledger'
import { readForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import { getStoryboardStory, setStoryArchitectBrief, setStoryScoutPacket } from '../../db/storyboard'
import { parseExecutionEnvironment } from '../../lib/execution-target'
import { assessSmithWork } from './forge-dispatch-seam'
import { findLatestLeadPlan, leadPreDispatchHoldReasons, parseLeadPlan, renderSmithWorkOrders } from './forge-lead-plan'
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

    // #8 context-lesson feedback loop: inject known context gaps for this node/lane
    // so a fact that was once missing is never missing twice. Null when no lessons
    // match, so a clean run pays nothing for the store.
    const contextLessonsDirective = buildContextLessonDirective(
      lessonsForArea(
        new FileContextLessonStore(
          process.env.FORGE_CONTEXT_LESSONS_FILE ??
            resolve(process.cwd(), '.forge-context', 'lessons.json'),
        ).list(),
        [nodeId, plan.lane],
      ),
    )

    // Phase 3 (ENG-FORGE-LEAD-WORKORDER): a serial Smith lane executes Lead's
    // persisted WORK ORDERS verbatim instead of re-deriving them. SPLIT child
    // lanes (smith_split_work) get their branch assignment and are not fed the
    // whole serial plan here.
    const smithLeadPlan =
      plan.lane === 'smith' && nodeId !== 'smith_split_work'
        ? findLatestLeadPlan(await runs.listForStory(resolvedStory.id))
        : null
    const extraInstructions = [
      correctiveNote,
      identityInstruction,
      branchInstruction,
      plan.evidenceInstruction,
      repoContextInstruction,
      priorScoutInstruction,
      contextLessonsDirective,
      buildRunGuardrailsDirective(),
      buildRunPassDirective(),
      buildRtkCompressionDirective(),
      plan.lane === 'smith'
        ? smithLeadPlan
          ? renderSmithWorkOrders(smithLeadPlan)
          : buildSmithWorkDecompositionDirective()
        : null,
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
      // FEATURE path: the engine already decided Scout is not needed for this
      // architect node (feature_scout_needed -> architect). Waive the Scout
      // packet requirement here ONLY for the feature architect; research and
      // repair architect nodes keep their Scout-packet gate.
      session:
        plan.lane === 'architect' && nodeId === 'architect'
          ? { scoutWaived: true }
          : undefined,
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
      // Anti-token-fire (Smith decomposition): a successful Smith that self-sizes
      // OVERSIZED or >3 chunks is a scope violation -> HOLD. A 4th chunk is not
      // "keep working". Adjudicated by the dispatch seam (the running enforcement
      // seat of the KRAKEN gate), which uses the machine SMITH_PLAN line if the
      // Smith emitted one, and upgrades to the full gate when a structured plan
      // is ever captured.
      if (plan.lane === 'smith') {
        const smithWork = assessSmithWork(raw)
        if (smithWork.verdict === 'HOLD') missing.push(...smithWork.reasons)
      }
      // Pre-Smith handoff gate (LEAD_PLAN contract): a successful Lead that
      // decided to dispatch to Smith (SMITH/SPLIT) MUST have emitted an
      // assessable LEAD_PLAN the KRAKEN gate accepts — otherwise it HOLDS and
      // never hands off, so the engine never starts a Smith lane. Absent plan
      // (NO_PLAN) IS gated (authoritative pre-Smith fuse). The gate decision is
      // persisted durably on the story run so audits and future gates read it
      // without parsing thrown errors.
      if (nodeId === 'lead_pre') {
        const leadHolds = leadPreDispatchHoldReasons(evidence.leadDecision, raw)
        const storyRunId = finishedItem?.storyRunId ?? null
        if (storyRunId) {
          await appendForgeRunDetail(
            storyRunId,
            `dispatch gate node=lead_pre decision=${evidence.leadDecision ?? '(none)'} ` +
              `verdict=${leadHolds.length > 0 ? 'HOLD' : 'GO'} ` +
              `reasons=${leadHolds.length > 0 ? leadHolds.join(' | ') : 'none'}`,
          ).catch(() => {
            /* run-detail append is observer-only; the gate verdict stands on the HOLD/GO above */
          })
          // Phase 2 (ENG-FORGE-LEAD-WORKORDER): persist the parsed plan durably
          // (work orders -> Smith) so a later Smith reads the SAME structured plan
          // Lead wrote, never a re-derivation from a notes blob.
          const plan = parseLeadPlan(raw)
          if (plan) {
            await appendForgeRunDetail(
              storyRunId,
              `lead-plan:${JSON.stringify(plan)}`,
            ).catch(() => {
              /* observer-only */
            })
          }
        }
        if (leadHolds.length > 0) missing.push(...leadHolds)
      }
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
