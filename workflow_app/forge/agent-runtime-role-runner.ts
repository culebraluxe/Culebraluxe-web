import { buildLaneEnqueue } from '../../agent-runtime/enqueue-lane'
import { buildRunGuardrailsDirective } from '../../agent-runtime/run-guardrails'
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
import { getStoryboardStory } from '../../db/storyboard'
import { parseExecutionEnvironment } from '../../lib/execution-target'
import { interactiveSql } from '../../lib/neon-interactive'
import type { ForgeRoleRunner } from './forge-executor'
import {
  forgeEvidenceFromAgentResult,
  forgeRoleNodePlan,
} from './forge-role-mapping'
import {
  findingsFromArchitectEvidence,
  validateLeadShapeChoice,
} from './forge-shaping'
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
      identityInstruction,
      branchInstruction,
      plan.evidenceInstruction,
      repoContextInstruction,
      priorScoutInstruction,
      buildRunGuardrailsDirective(),
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
    await linkForgeEngineTaskExecution({
      taskId: task.taskId,
      processInstanceId: task.processInstanceId,
      tokenId: task.tokenId,
      storyId: resolvedStory.id,
      nodeId,
      workItemId: queued.id,
      workerId: options.workerId,
    })
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

    // ENG-FORGE-SHAPE-01: persist Architect findings durably (so Lead can gate on
    // the REAL Architect output), then enforce the shaping gate at Lead completion.
    const isArchitectNode =
      nodeId === 'architect' || nodeId === 'repair_architect' || nodeId === 'research_architect'
    if (isArchitectNode) {
      const parsedFindings = findingsFromArchitectEvidence(
        [result.evidence.notes, result.evidence.testsSummary].filter(Boolean).join('\n'),
      )
      if (parsedFindings.length > 0) evidence.findings = parsedFindings
    } else if (nodeId === 'lead_pre' && evidence.leadDecision) {
      const findings = current.findings ?? []
      if (findings.length > 0) {
        const verdict = validateLeadShapeChoice({
          findings,
          choice: evidence.leadDecision,
          splitCount: evidence.splitCount,
        })
        if (!verdict.ok) {
          // Authoritative override: e.g. Lead said single SMITH over independent
          // seams -> route the engine to the bounded SPLIT the findings demand.
          evidence.leadDecision = verdict.authoritative.decision
          evidence.splitCount = verdict.authoritative.splitCount ?? evidence.splitCount
        }
      }
    }

    await finishForgeEngineTaskExecution(task.taskId, {
      storyRunId: finishedItem?.storyRunId ?? null,
      status: /pass|success|complete/i.test(result.evidence.resultStatus)
        ? 'completed'
        : 'failed',
    })
    return {
      transitionName: 'complete',
      evidence,
    }
  }
}
