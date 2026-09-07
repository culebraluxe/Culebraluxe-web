import { buildLaneEnqueue } from '../../agent-runtime/enqueue-lane'
import { createAgentRuntimeRegistry, parseBuilderFlashOverride } from '../../agent-runtime/factory'
import {
  buildAgentInvokerWorkspaces,
  executeClaimedAgentCommand,
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
import { getAgentWorkItem } from '../../db/agent-work'
import {
  finishForgeEngineTaskExecution,
  linkForgeEngineTaskExecution,
} from '../../db/forge-engine-task-execution'
import { getForgeLeadRunRecord } from '../../db/forge-run'
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
  readLegacyMarkerGateEvidence,
  readTypedGateEvidence,
} from './forge-typed-evidence'
import type { ForgeGateEvidence } from './forge-facts'

export type AgentRuntimeForgeRunnerOptions = {
  workerId: string
  executionEnvironment?: string | null
}

const SCOUT_RESEARCH_CONSUMERS = new Set(['architect', 'lead', 'smith', 'inspector'])

export function createAgentRuntimeForgeRoleRunner(
  options: AgentRuntimeForgeRunnerOptions,
): ForgeRoleRunner {
  const work = new SqlAgentWorkRepository(async () => interactiveSql as never)
  const runs = new SqlAgentRunRepository(async () => interactiveSql as never)
  const registry = createAgentRuntimeRegistry({
    builderFlashOverride: parseBuilderFlashOverride(
      process.env.FORGE_PROVIDER_BUILDER_FLASH ?? null,
    ),
  })
  const workspaces = buildAgentInvokerWorkspaces(options.workerId)

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

    // REPO_CONTEXT — Scout gets a deterministic structural map before it burns
    // model tokens grepping/opening files. Ripwire failure is fail-soft: the
    // Scout still runs and must produce its durable research handoff.
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

    // Scout output is already durable Story Run evidence in Neon. Feed the
    // latest synthesis forward to judgment/build lanes as evidence, never as
    // authority. Assay intentionally does NOT consume model prose.
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

    // A claim transition may return a deliberately narrow projection of the
    // work row. Runtime launch guards, however, require the COMPLETE durable
    // execution contract (notably execution_environment / execution_policy).
    // Re-read the just-claimed item from canonical storage before launch so the
    // runtime never executes from a partial SQL RETURNING projection.
    const durableClaim = await work.get(queued.id)
    if (!durableClaim) {
      throw new Error(
        `Forge engine task ${task.taskId} claimed agent work item ${queued.id}, but the durable row could not be reloaded`,
      )
    }

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
