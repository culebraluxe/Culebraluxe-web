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
import {
  listSplitChildOutcomes,
  recordSplitChildAssignment,
  recordSplitChildCandidate,
} from '../../db/forge-split-children'
import { splitJoinHoldReasons } from './split-join'
import { getStoryboardStory, setStoryArchitectBrief, setStoryScoutPacket } from '../../db/storyboard'
import { parseExecutionEnvironment } from '../../lib/execution-target'
import { assessSmithWork, smithDispatchRunDetail } from './forge-dispatch-seam'
import { renderSmithWorkOrders } from './forge-lead-plan'
import { leadRoutingFacts, parseLeadRouting, reviewLeadProposal } from './forge-lead-routing'
import { buildLeadRoutingDirective } from './forge-lead-routing-prompt'
import { renderSplitAssignmentWorkOrders, smithContractFromAssignment, splitChildAssignment } from './forge-split-handoff'
import { scopeViolations, type SmithExecutionContract } from './smith-contract'
import { changedFilesForCandidate } from '../../lib/worker-workspace/candidate-diff'
import { deriveWorktreePath } from '../../lib/worker-workspace/provisioner'
import {
  acceptedLeadRouting,
  buildLeadRoutingContext,
  findLatestAcceptedLeadRouting,
  type LeadRoutingCapabilities,
} from './lead-routing-context'

/**
 * Trusted runtime capability for LEAD routing.
 *
 * SPLIT IS ENABLED BY DEFAULT (captain's call, 2026-09-10) now that the lane is
 * proven end to end: Lead SPLIT accepted → real fork → two children running
 * concurrently in their own worktrees, each producing its OWN candidate SHA →
 * satisfied join → `lead_post` integrating → QA PASS. Evidence is durable
 * (forge_workflow_evidence: lead_decision=SPLIT, split_count=2, lead_routing with 2
 * assignments, per-child candidate_shas on each child row).
 *
 * The door can still be closed explicitly per environment with
 * `FORGE_SPLIT_ENABLED=false` (fail-closed switch), and the runtime — never the
 * model — owns these numbers.
 *
 * Knobs: FORGE_SPLIT_ENABLED (default on) · FORGE_SPLIT_MAX_SMITHS (default 2)
 * · FORGE_SPLIT_CONCURRENCY (worker, default 2)
 */
const LEAD_ROUTING_CAPABILITIES: LeadRoutingCapabilities = {
  splitEnabled: process.env.FORGE_SPLIT_ENABLED !== 'false',
  maxSmiths: Math.max(1, Math.trunc(Number(process.env.FORGE_SPLIT_MAX_SMITHS ?? '2')) || 2),
}
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
    // Trusted LEAD routing context (Astra handoff): derived from durable story +
    // Architect findings and runtime capability — NEVER from model output. Read
    // once per attempt; the same `current` is reused for gate evidence below.
    const current = await readForgeWorkflowEvidence(resolvedStory.id)
    const leadRoutingContext = buildLeadRoutingContext({
      story: resolvedStory,
      findings: current.findings,
      capabilities: LEAD_ROUTING_CAPABILITIES,
    })
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

    // Astra handoff: the worker that will EXECUTE Lead's accepted work orders gets
    // exactly that assignment's plan — the serial Smith lane OR the Lead SOLO
    // implementation lane (SOLO needs the work orders too; that injection used to
    // be Smith-only). A SPLIT child gets ITS OWN assignment resolved from the
    // re-validated accepted proposal against the engine's 0-based branch index,
    // cross-checked with the slice the engine handed it. A child that cannot be
    // tied to an accepted assignment HOLDS before launch: handing it the general
    // decomposition directive would let it invent its own scope (that is how a
    // branch gets mangled and a candidate ends up from the wrong workspace).
    const acceptedRouting = acceptedLeadRouting({
      durable: current.leadRouting,
      runs: await runs.listForStory(resolvedStory.id),
      context: leadRoutingContext,
    })
    const splitChildContract =
      nodeId === 'smith_split_work'
        ? splitChildAssignment({ proposal: acceptedRouting, formData: task.formData })
        : null
    if (splitChildContract && (splitChildContract.errors.length > 0 || !splitChildContract.assignment)) {
      throw new Error(
        `Forge ${nodeId} HOLD: ${splitChildContract.errors.join('; ') || 'no resolvable Lead assignment'}`,
      )
    }
    // The assignment's execution contract must be VALID before the child launches:
    // identity (story/node/attempt/owner), allowedScope from the plan's surfaces,
    // requiredEvidence from the plan's proofs, and the SIBLING surfaces this child may
    // not touch in prohibitedScope. Without this gate the contract is library-only and
    // a malformed assignment could still put a child in a worktree with no enforced
    // boundary — the mangled-branch failure mode.
    let splitAssignmentContract: SmithExecutionContract | null = null
    if (splitChildContract?.assignment) {
      const splitAssignment = splitChildContract.assignment
      // Sibling surfaces become the child's prohibitedScope: the boundary is then
      // machine-enforced by the contract, not merely stated in prose.
      const siblings = (acceptedRouting?.assignments ?? []).filter((a) => a.id !== splitAssignment.id)
      const built = smithContractFromAssignment({
        storyId: resolvedStory.id,
        nodeId,
        // The loop counter is 0-based; a contract attempt is 1-based (validateSmithContract).
        attempt: attempt + 1,
        assignment: splitAssignment,
        siblings,
      })
      if (built.errors.length > 0) {
        throw new Error(
          `Forge ${nodeId} HOLD: split assignment contract invalid — ${built.errors.join('; ')}`,
        )
      }
      splitAssignmentContract = built.contract
    }
    // The join is the engine's, but INTEGRATION is ours: `lead_post` must not
    // integrate a fan-out whose children did not all finish with their own
    // candidate SHA (compute it before the model runs, so a bad join costs no tokens).
    if (nodeId === 'lead_post' && acceptedRouting?.decision === 'SPLIT') {
      const expectedIds = acceptedRouting.assignments.map((a) => a.id)
      const { outcomes, unrecorded } = await listSplitChildOutcomes(resolvedStory.id, {
        groupId: task.processInstanceId,
      })
      const holdReasons = splitJoinHoldReasons({ expectedIds, outcomes, unrecordedCandidates: unrecorded })
      if (holdReasons.length > 0) {
        throw new Error(`Forge ${nodeId} HOLD: SPLIT join not satisfied — ${holdReasons.join('; ')}`)
      }
    }
    const executesLeadWorkOrders =
      (plan.lane === 'smith' && nodeId !== 'smith_split_work') || nodeId === 'lead_solo_implement'
    const acceptedLeadPlan = executesLeadWorkOrders ? (acceptedRouting?.assignments[0]?.plan ?? null) : null
    // When Astra routing governs PRE, the legacy lead_pre evidence contract
    // (FORGE_EVIDENCE_JSON.leadDecision/splitCount + LEAD_PLAN) must NOT be injected:
    // on a live run the model obeyed the longer legacy text, emitted no LEAD_ROUTING
    // line at all, and HOLDed. The routing directive is the only routing contract in
    // that lane; the legacy text still applies to lanes without a routing context.
    const leadRoutingGovernsPre = nodeId === 'lead_pre' && Boolean(leadRoutingContext)
    const extraInstructions = [
      correctiveNote,
      identityInstruction,
      branchInstruction,
      leadRoutingGovernsPre ? null : plan.evidenceInstruction,
      repoContextInstruction,
      priorScoutInstruction,
      contextLessonsDirective,
      buildRunGuardrailsDirective(),
      buildRunPassDirective(),
      buildRtkCompressionDirective(),
      // SPLIT child: execute ONLY its own accepted assignment. The general
      // decomposition directive is deliberately unreachable here (see the
      // fail-closed resolution above).
      nodeId === 'smith_split_work'
        ? renderSplitAssignmentWorkOrders(splitChildContract!.assignment!)
        : plan.lane === 'smith'
          ? acceptedLeadPlan
            ? renderSmithWorkOrders(acceptedLeadPlan)
            : buildSmithWorkDecompositionDirective()
          : nodeId === 'lead_solo_implement' && acceptedLeadPlan
            ? renderSmithWorkOrders(acceptedLeadPlan)
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
      leadRoutingContext,
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
      // SPLIT children are a parallel group, not serial work: without this they are
      // governed by the one-serial-active-per-story index and siblings collapse
      // onto a single work item (the second child then cannot be claimed).
      ...(splitChildContract?.index !== null && splitChildContract?.index !== undefined
        ? { parallelGroupId: task.processInstanceId, parallelSlot: splitChildContract.index + 1 }
        : {}),
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
    if (splitChildContract?.assignment && splitChildContract.index !== null) {
      // Provenance (ENG-FORGE-SPLIT-01): tie this child's durable row to the ONE
      // accepted assignment it owns before it runs, so the join can be accounted
      // per child. No silent catch — an untraceable child must not be admitted.
      await recordSplitChildAssignment(durableClaim.id, {
        assignmentId: splitChildContract.assignment.id,
        index: splitChildContract.index,
      })
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
    // Provenance (ENG-FORGE-SPLIT-01): a split child's candidate SHA is recorded
    // against ITS OWN work item, so the join can never credit a sibling's SHA or
    // the parent checkout's. No silent catch: if the write fails the run fails
    // closed rather than letting an untraceable candidate reach the join.
    if (nodeId === 'smith_split_work' && typeof evidence.candidateSha === 'string' && evidence.candidateSha.trim()) {
      // The child's candidate must be INSIDE its assignment. Declared scope is checked
      // against the child's real diff before its SHA is accepted — otherwise the join
      // only proves "every child produced a SHA", not "that SHA was in its lane".
      // Fail-closed: an unresolvable diff or any out-of-scope path refuses the child.
      if (splitAssignmentContract) {
        const cwd = workspaces?.worktreesRoot
          ? deriveWorktreePath(workspaces.worktreesRoot, resolvedStory.id, executionId)
          : process.cwd()
        const changedFiles = await changedFilesForCandidate({
          cwd,
          baseRef: workspaces?.baseRef ?? 'origin/main',
          candidateSha: evidence.candidateSha,
        })
        const violations = scopeViolations(splitAssignmentContract, changedFiles)
        if (violations.length > 0) {
          throw new Error(
            `Forge ${nodeId} HOLD: candidate ${evidence.candidateSha.slice(0, 12)} touched files outside its assignment (${splitAssignmentContract.identity.owner}): ${violations.join(', ')}`,
          )
        }
      }
      await recordSplitChildCandidate(durableClaim.id, evidence.candidateSha)
    }
    // LEAD routing (Astra handoff): the AI proposes, code accepts or returns
    // corrections — it never silently reroutes. The validated proposal is the
    // single source of engine routing facts, and a malformed/absent proposal is a
    // miss that rides the bounded self-heal path below (never a default route).
    const routingReview = nodeId === 'lead_pre' ? reviewLeadProposal(parseLeadRouting(raw), leadRoutingContext) : null
    if (routingReview?.ok) {
      Object.assign(evidence, leadRoutingFacts(routingReview))
    }

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

    // LEAD routing validation is a PREREQUISITE for handing off execution, not an
    // optional formatting check — so it applies even when deliverable enforcement
    // is disabled (FORGE_ENFORCE_DELIVERABLES=0). Errors ride the same bounded
    // self-heal path below; exhaustion is a real HOLD.
    if (successful && routingReview && !routingReview.ok) {
      miss.push(...routingReview.errors.map((reason) => `lead-routing:${reason}`))
    }
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
        if (smithWork.verdict === 'HOLD') {
          missing.push(...smithWork.reasons)
          // Final-attempt observability (ENG-FORGE-V14): persist the full
          // dispatch-gate adjudication on the story run before the HOLD throw
          // below carries the same reasons in `miss`. Observer-only .catch,
          // mirroring the lead_pre record; self-heal attempts do not duplicate.
          if (attempt + 1 >= totalAttempts) {
            const storyRunId = finishedItem?.storyRunId ?? null
            if (storyRunId) {
              await appendForgeRunDetail(storyRunId, smithDispatchRunDetail(smithWork)).catch(
                () => {
                  /* run-detail append is observer-only; the HOLD throw below stands */
                },
              )
            }
          }
        }
      }
      // Durable observability of the PRE routing verdict (NOT a second gate).
      //
      // HISTORY (do not be misled by older revisions of this comment): a
      // "Pre-Smith handoff gate (LEAD_PLAN contract)" once lived here and HOLDS'd
      // any SMITH/SPLIT route without an assessable LEAD_PLAN. That fuse was
      // RETIRED when Astra LEAD routing landed: the pre-Smith brake is now
      // `reviewLeadProposal`'s structural gate (forge-lead-routing.ts), which
      // validates each assignment's plan (chunk ceiling, surfaces, proofs drawn
      // from the frozen acceptance) before Smith can start. `assessLeadHandoff`
      // in forge-lead-plan.ts therefore has NO production caller. The block below
      // only records the verdict; it no longer gates the handoff.
      if (nodeId === 'lead_pre') {
        // The routing decision itself is owned by reviewLeadProposal above; this is
        // durable observability of its verdict, not a second gate.
        if (nodeId === 'lead_pre') {
          const storyRunId = finishedItem?.storyRunId ?? null
          if (storyRunId && routingReview) {
            await appendForgeRunDetail(
              storyRunId,
              `lead-routing verdict=${routingReview.ok ? 'GO' : 'HOLD'} ` +
                `errors=${routingReview.ok ? 'none' : routingReview.errors.join(' | ')} ` +
                `advisories=${routingReview.ok && routingReview.advisories.length ? routingReview.advisories.join(' | ') : 'none'}`,
            ).catch(() => {
              /* run-detail append is observer-only; the routing verdict stands above */
            })
          }
        }
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
      correctiveNote = buildSelfHealDirective(
        nodeId,
        miss,
        leadRoutingGovernsPre && leadRoutingContext
          ? buildLeadRoutingDirective(leadRoutingContext)
          : plan.evidenceInstruction,
      )
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
