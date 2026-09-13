import { resolve } from 'node:path'
import { buildLaneEnqueue } from '../../agent-runtime/enqueue-lane'
import { buildBrevityDirective, buildGroundingDirective, buildRunGuardrailsDirective, buildRunPassDirective, buildRtkCompressionDirective } from '../../agent-runtime/run-guardrails'
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
import { readForgeWorkflowEvidence, mergeForgeWorkflowEvidence } from '../../db/forge-workflow-evidence'
import {
  listSplitChildOutcomes,
  recordSplitChildAssignment,
  recordSplitChildCandidate,
} from '../../db/forge-split-children'
import { splitJoinHoldReasons } from './split-join'
import { getStoryboardStory, setStoryArchitectBrief, setStoryScoutPacket } from '../../db/storyboard'
import { assertForgeExecutionTarget, assertForgeLaneMayStart } from './forge-execution-target'
import { assessSmithWork, smithDispatchRunDetail } from './forge-dispatch-seam'
import { assessArchitectBrief } from './forge-shaping'
import { renderSmithWorkOrders } from './forge-lead-plan'
import { leadRoutingFacts } from './forge-lead-routing'
import { buildLeadRoutingDirective } from './forge-lead-routing-prompt'
import type { RoleEffectPorts } from './agents/ports'
import { existsOnGitBaseRef } from './agents/architect/exists-git'
import { parseArchitectHandoff } from './agents/architect-handoff'
import { seamGroupHint } from './agents/architect/shape-hint'
import { smithWorkOrdersFromFindings } from './agents/architect/persist'
import { assignmentFromLead } from './agents/smith/from-lead'
import { buildSmithDirective } from './agents/smith/prompt'
import { buildSelfHealDirectiveWithReasons } from './agents/self-heal'
import { getForgeRoleContract, type ForgeRoleContract } from '../../db/forge-role-contract'
import { getForgeRolePlan, type ForgeRolePlan } from '../../db/forge-role-plan'
import { resolveLeadProposal } from './lead-proposal-resolve'
import { buildArchitectDirective } from './forge-architect-directive'
import { assessSmithExit } from './smith-candidate'
import { commandRunner, staticSliceForWorktree } from './agents/exec-command'
import { renderSplitAssignmentWorkOrders, smithContractFromAssignment, splitChildAssignment } from './forge-split-handoff'
import type { SmithExecutionContract } from './smith-contract'
import { createPersistentTraceSink } from './forge-observer'
import {
  drainAlerts,
  observeAttemptBegin,
  observeAttemptEnd,
  observeCandidateCommit,
  observeHold,
  observeMeasurementGap,
  observeRouteHold,
  type AttemptStatus,
} from './forge-observer-seam'
import { serialLaunchDoor, serialScopeMissReasons } from './forge-serial-doors'
import { recordTraceEvent, listTraceEvents } from '../../db/workflow-trace'
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

/**
 * Forge Observer — PHASE 1: RECORD ONLY.
 *
 * Instruments are not authority. This sink never routes, never throws and never
 * gates a run: it writes the worker-execution layer (tool/file/scope/hold/alert
 * events) that nothing recorded before, through the EXISTING trace table
 * (workflow_execution_trace_event) rather than a second store. Alerts are
 * classified as `hold-recommend` at most — the runner still owns HOLD.
 *
 * One sink per process, so alert rules can compare events across attempts
 * (see RETRY_UNCHANGED_INPUT in forge-alerts/rules.ts).
 *
 * FORGE-OBS-LIST-01: "one per process" was also the bug. The sink starts empty on
 * every process, so an attempt that began before a restart was invisible — the
 * retry hash had nothing to compare against, and drainAlerts re-recorded every
 * alert as new because its de-dupe set (sink.list) was empty too. `read` plus
 * `load()` below gives the sink the story's persisted history back, from the SAME
 * trace table it writes to, so the fix is not a second store.
 */
const forgeObserverSink = createPersistentTraceSink({
  write: (input) => recordTraceEvent(input as never),
  read: (key) => listTraceEvents({ workflowInstanceId: key.processInstanceId, limit: 5000 }),
})
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
  /**
   * Operator launch cap for THIS dispatch (migration 167, `agent_work_item.launch_intent`).
   * Null means the Lead decides, as it always has. When set it rides the role-effect
   * ports as `benchIntent`, where the Lead's own cap check enforces it — so the cap is
   * applied by the same code that already owns that rule, not by a second one here.
   */
  launchIntent?: 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | null
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
    // ---------------------------------------------------------------------
    // ENG-FORGE-SYNC-GUARD-01 — a lane cannot start anywhere but PROD.
    //
    // FIRST statement on purpose. `driveForgeStory` calls this runner for every
    // node of every lane (forge-executor.ts:222), so this is the choke point
    // where "do not claim a task, spawn OpenCode or provision a worktree" can
    // actually be guaranteed. What stood here before was a
    // `parseExecutionEnvironment(..., 'DEV')` default: silence meant DEV, the
    // work item recorded DEV, and the mismatch stayed invisible until the board
    // disagreed with git.
    // ---------------------------------------------------------------------
    if (options.executionEnvironment) {
      // A caller-declared target is still a claim about where this lane runs.
      assertForgeExecutionTarget(options.executionEnvironment)
    }
    const laneTarget = assertForgeLaneMayStart({ env: process.env })

    const subjectRows = await interactiveSql`
      select subject_id
      from process_instances
      where id = ${task.processInstanceId}
      limit 1
    `
    const resolvedStory = await getStoryboardStory(String(subjectRows[0]?.subject_id ?? ''))
    if (!resolvedStory) throw new Error(`Forge engine task ${task.taskId} has no Storyboard story`)

    // ---------------------------------------------------------------------
    // BATCH-SLICED ROLLOUT — DEV_OPS IS HELD OFF THE CHAIN (migration 148).
    //
    // A story flagged `batch_deploy` belongs to a major feature rollout that must
    // deploy as ONE deliberate slice, not story-by-story. Satisfying the receipt
    // gate is NOT enough: the DEV_OPS lane would still publish and (because main
    // maps to production) deploy. So for these stories the release lane does not
    // run at all — no publish, no deploy, no fabricated receipt — and the node
    // completes with an honest DEFERRAL record instead.
    //
    // Release deliberately: scripts/forge-batch-release.mjs (lists the slice), then
    // the real publish/deploy/verify for the whole batch.
    // ---------------------------------------------------------------------
    if (resolvedStory.batchDeploy && (nodeId === 'deploy' || nodeId === 'production_smoke')) {
      const deferredTo = resolvedStory.batch ?? 0
      await mergeForgeWorkflowEvidence(task.processInstanceId, resolvedStory.id, {
        deploymentDeferredToBatch: deferredTo,
      })
      await finishForgeEngineTaskExecution(task.taskId, { storyRunId: null, status: 'completed' })
      console.log(
        `[${nodeId}] batch-sliced rollout: DEV_OPS held off the chain — deployment DEFERRED to batch ${deferredTo}. Nothing published or deployed.`,
      )
      return { transitionName: 'complete', evidence: { deploymentDeferredToBatch: deferredTo } }
    }

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
    // The model has no memory between attempts. This carries its OWN last reply so a
    // retry repairs rather than rewrites (see self-heal.ts).
    let previousReply: string | null = null
    for (let attempt = 0; attempt < totalAttempts; attempt++) {
    const plan = forgeRoleNodePlan(nodeId)
    // Trusted LEAD routing context (Astra handoff): derived from durable story +
    // Architect findings and runtime capability — NEVER from model output. Read
    // once per attempt; the same `current` is reused for gate evidence below.
    const current = await readForgeWorkflowEvidence(resolvedStory.id)
    // Batch-sliced rollout (migration 148): record the deferral as soon as the story
    // is known to be a batch story — NOT only at the deploy node — because definition
    // v6's qa_result decision reads `releaseDeferred` to hold the whole release tail
    // (publish included) and complete the story QA-verified. Recording the deferral is
    // honest; recording a deployment/publish receipt here would be a fabrication.
    if (resolvedStory.batchDeploy && current.deploymentDeferredToBatch == null) {
      await mergeForgeWorkflowEvidence(task.processInstanceId, resolvedStory.id, {
        deploymentDeferredToBatch: resolvedStory.batch ?? 0,
      })
    }
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

    // The REPO INDEX goes to the roles that must know scope, not just to Scout.
    // It was injected for `scout` only, so the Architect — which the FEATURE path
    // reaches WITHOUT a scout packet — had neither findings nor index, and did the
    // only thing left: read the repo by hand. That is where 13 minutes went on a
    // one-line change. The index is the thing that stops exploration.
    const repoContextInstruction =
      plan.lane === 'scout' || plan.lane === 'architect'
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
    // ENG-FORGE-OBS-SERIAL-01: the SERIAL lane's declared scope.
    //
    // A split child is handed the assignment it must execute; the serial Smith /
    // lead_solo_implement lane executes the SAME accepted assignment (the line
    // above reads assignments[0].plan) but its contract was never built, so
    // `isChangeAllowed` had no caller on the path that runs every story — the same
    // defect ENG-FORGE-SPLIT-01 fixed for children (see smith-contract.ts).
    //
    // MEASUREMENT ONLY. This contract exists to judge the candidate, not to gate
    // it: turning serial declared-scope into a HOLD would add a new gate to the
    // lane every story runs, and this story's scope says not to change HOLD
    // policy. An unbuildable contract is skipped for the same reason.
    const serialAssignment = executesLeadWorkOrders ? (acceptedRouting?.assignments[0] ?? null) : null
    let serialAssignmentContract: SmithExecutionContract | null = null
    if (serialAssignment) {
      const builtSerial = smithContractFromAssignment({
        storyId: resolvedStory.id,
        nodeId,
        attempt: attempt + 1,
        assignment: serialAssignment,
        siblings: (acceptedRouting?.assignments ?? []).filter((a) => a.id !== serialAssignment.id),
      })
      if (builtSerial.errors.length === 0) serialAssignmentContract = builtSerial.contract
    }
    // ---------------------------------------------------------------------
    // DOOR 1 — SERIAL SMITH LAUNCH (FORGE-SMITH-DOOR-01)
    //
    // Captain: "Lead can make those decisions, not Smith." A lane that executes
    // work orders cannot start without an accepted Lead assignment; this lane used
    // to fall back to the WORK-DECOMPOSITION directive, which let Smith size, chunk
    // and effectively choose its own work. The rule lives in forge-serial-doors.ts
    // so it cannot drift from the scope door below (they were two separate commits
    // and Grok's review flagged exactly that drift risk).
    // ---------------------------------------------------------------------
    const launchDoor = serialLaunchDoor({
      nodeId,
      hasAcceptedAssignment: Boolean(serialAssignment),
    })
    if (!launchDoor.allowed) throw new Error(String(launchDoor.reason))

    // When Astra routing governs PRE, the legacy lead_pre evidence contract
    // (FORGE_EVIDENCE_JSON.leadDecision/splitCount + LEAD_PLAN) must NOT be injected:
    // on a live run the model obeyed the longer legacy text, emitted no LEAD_ROUTING
    // line at all, and HOLDed. The routing directive is the only routing contract in
    // that lane; the legacy text still applies to lanes without a routing context.
    const leadRoutingGovernsPre = nodeId === 'lead_pre' && Boolean(leadRoutingContext)
    // The Architect's OWN words for the findings this lane owns: the live
    // renderSmithWorkOrders states the PLAN (chunks, scope, proofs); this states
    // the preconditions, postconditions, classes and risks the Architect attached
    // to exactly those findings, read off the handoff rather than paraphrased.
    const smithFindingsInstruction = (() => {
      if (!executesLeadWorkOrders || !serialAssignment) return null
      const handoff = parseArchitectHandoff(resolvedStory.architectBrief ?? '')
      const findingIds = serialAssignment.findingIds ?? []
      if (!handoff || findingIds.length === 0) return null
      return smithWorkOrdersFromFindings(findingIds, handoff)
    })()

    const extraInstructions = [
      correctiveNote,
      identityInstruction,
      branchInstruction,
      smithFindingsInstruction,
      // The Architect's OWN directive, carrying the story's frozen proofs.
      //
      // The run's pinned SHA reaches the model through the ISOLATION instruction
      // (added at provisioning, after this list is assembled), so we pass an empty
      // baseRef: buildArchitectDirective then instructs the Architect to name the
      // SHA it inspected, and the handoff assessor validates the claim against that
      // SHA with git. Inventing "origin/main" here would name the WRONG object.
      plan.lane === 'architect' ? buildArchitectDirective('', leadRoutingContext.allowedProofs) : null,
      leadRoutingGovernsPre ? null : plan.evidenceInstruction,
      repoContextInstruction,
      priorScoutInstruction,
      contextLessonsDirective,
      // THE DECLARED SURFACES, said out loud.
      //
      // Discovery belongs to Scout (which gets the index). When the STORY already
      // names its files, no other role should be reading the repo "just in case" —
      // that behaviour is what turned a one-line change into 13 minutes. Stating the
      // surfaces is what removes the invitation to explore.
      resolvedStory.scope?.trim()
        ? `DECLARED SURFACES (authoritative — work within these; do NOT survey or glob the repository):\n${resolvedStory.scope.trim()}`
        : null,
      buildRunGuardrailsDirective(),
      buildBrevityDirective(),
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
            : // Unreachable: the serial-lane guard above HOLDs when the Lead has
              // routed nothing. Smith never sizes or scopes its own work.
              null
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

    // Resolved and asserted ONCE at lane start (top of this function): the work
    // item, the trace events and the run all carry this same value.
    const target = laneTarget
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
    if (
      splitChildContract?.assignment &&
      splitChildContract.index !== null &&
      splitChildContract.index !== undefined
    ) {
      // Provenance (ENG-FORGE-SPLIT-01): tie this child's durable row to the ONE
      // accepted assignment it owns before it runs, so the join can be accounted
      // per child. No silent catch — an untraceable child must not be admitted.
      await recordSplitChildAssignment(durableClaim.id, {
        assignmentId: splitChildContract.assignment.id,
        index: splitChildContract.index,
        // Same source as the enqueue's `parallelGroupId` above: one process
        // instance = one fan-out group, so both writers set an identical tuple.
        groupId: task.processInstanceId,
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
    const priorReply = previousReply
    previousReply = raw
    // ENG-FORGE-PHASE-AGENT: collect() is the subclass hook that FILLS evidence
    // (architect handoff parse, Lead routing, Smith candidate, Assay verdict,
    // DEV_OPS receipt). Only the effects the runner can honestly supply are
    // passed; an omitted port is skipped, and the parent gate below still HOLDs
    // when the corresponding field is absent. The parent remains the decider.
    // The candidate's OWN worktree, computed the same way the serial and split
    // lanes compute it. Assay commands and the static gate run THERE, never in
    // this checkout.
    const roleCwd = workspaces?.worktreesRoot
      ? deriveWorktreePath(workspaces.worktreesRoot, resolvedStory.id, executionId)
      : process.cwd()
    // The candidate diff, read HERE rather than trusting the model's claim. It is
    // the same measurement the serial/split lanes make later; git's diff is the
    // authority for what a Smith actually touched.
    const collectCandidateSha =
      typeof evidence.candidateSha === 'string' && evidence.candidateSha.trim()
        ? evidence.candidateSha.trim()
        : null
    const collectMergeBase = workspaces?.baseRef ?? 'origin/main'
    const collectDiff = collectCandidateSha
      ? await changedFilesForCandidate({
          cwd: roleCwd,
          baseRef: collectMergeBase,
          candidateSha: collectCandidateSha,
        }).then(
          (changedPaths) => ({
            candidateSha: collectCandidateSha,
            mergeBase: collectMergeBase,
            changedPaths,
          }),
          () => undefined,
        )
      : undefined
    // The Lead's recorded DECISION rows (migrations 170/171), read before the review
    // below — which is the ONE seat for this decision. `LeadAgent.collect` is a no-op
    // for PRE, so there is no second evaluator that could accept what this refused.
    const recordedContract: ForgeRoleContract | null =
      nodeId === 'lead_pre'
        ? await getForgeRoleContract({ taskId: task.taskId, nodeId, attempt: attempt + 1 })
        : null
    const recordedPlan: ForgeRolePlan | null =
      nodeId === 'lead_pre'
        ? await getForgeRolePlan({ taskId: task.taskId, nodeId, attempt: attempt + 1 })
        : null
    const rolePorts: RoleEffectPorts = {
      splitEnabled: leadRoutingContext.splitEnabled,
      maxSmiths: leadRoutingContext.maxSmiths,
      allowedProofs: leadRoutingContext.allowedProofs,
      evidenceRefs: leadRoutingContext.evidenceRefs,
      baseRef: workspaces?.baseRef ?? undefined,
      // The operator's launch cap for THIS dispatch (migration 167). Absent means
      // "the Lead decides", which is the behaviour every run has had until now.
      ...(options.launchIntent ? { benchIntent: options.launchIntent } : {}),
      repoDir: roleCwd,
      // Fail-closed seam check: a file the Architect names must exist on the
      // pinned baseRef (git cat-file against the sha).
      existsOnBaseRef: existsOnGitBaseRef(roleCwd),
      // The Assay lane verifies with the story's FROZEN proofs, in the candidate
      // worktree, plus the live static gate (architecture is the hard gate and a
      // SKIPPED arch gate is not a fail).
      assayCommands: leadRoutingContext.allowedProofs,
      runCommand: commandRunner(roleCwd),
      // The candidate worktree has the code but NO node_modules, so the hard arch
      // gate must be pointed at the PRIMARY checkout's binaries or it silently
      // "skips" — and a skipped arch gate is not a fail. process.cwd() is the
      // primary checkout; roleCwd is the candidate.
      runStatic: () => staticSliceForWorktree(roleCwd, process.cwd()),
      // A batch-sliced rollout is a RECORDED deferral, never a fake receipt.
      ...(resolvedStory.batchDeploy
        ? { deploymentDeferredToBatch: resolvedStory.batch ?? 0, deploymentRequired: false }
        : {}),
      // The candidate diff the runner measured itself, for the Smith scope lock.
      ...(collectDiff ? { runnerDiff: collectDiff } : {}),
      // The DURABLE release receipt, straight from workflow evidence. DEV_OPS
      // records a receipt it actually holds — never a fabricated one.
      ...(current.productionVerificationReceipt
        ? {
            releaseEvidence: {
              kind: 'production_verification' as const,
              success: true,
              receiptId: current.productionVerificationReceipt,
              artifactSha: current.productionVerifiedSha ?? null,
            },
          }
        : current.deploymentReceipt
          ? {
              releaseEvidence: {
                kind: 'deployment' as const,
                success: true,
                receiptId: current.deploymentReceipt,
                artifactSha: current.deployedSha ?? null,
              },
            }
          : {}),
      // benchIntent is deliberately NOT supplied. Bench membership does not imply a
      // launch cap — every non-null value in that type IS a cap, so deriving one
      // would ban SPLIT for every active story. It stays absent until the board
      // writes an explicit `launch_intent` (NULL = today's Lead behaviour).
    }
    // A rejection is a fact about THIS attempt, never inherited state: a stale
    // one from a previous role node would HOLD a role that did nothing wrong.
    delete evidence.deliverableRejection
    // collect() is PURE: it returns a new evidence object. The old
    // marshalFindings mutated in place, so discarding this return would silently
    // drop every field the subclasses fill — the wiring would look right and do
    // nothing.
    Object.assign(evidence, agent.collect(evidence, raw, rolePorts))

    // OBSERVER (phase 1, record-only): EVERY role attempt, not just split
    // children — this is what makes the worker-execution layer live in normal
    // traffic instead of waiting for a SPLIT. The sink cannot throw and the
    // events never gate; the runner still decides everything below.
    const observerAttempt = {
      storyId: resolvedStory.id,
      processInstanceId: task.processInstanceId,
      taskId: durableClaim.id,
      nodeId,
      attempt: attempt + 1,
      worktreePath: roleCwd,
      baseCommit: workspaces?.baseRef ?? 'origin/main',
    }
    const candidateSha =
      typeof evidence.candidateSha === 'string' && evidence.candidateSha.trim()
        ? evidence.candidateSha
        : undefined
    // ENG-FORGE-OBS-SERIAL-01 box 1 — what the SERIAL lane's candidate did outside
    // its accepted assignment, carried to the HOLD/self-heal assembly below.
    let serialScopeMiss: string[] = []
    // FORGE-OBS-LIST-01 — give this process the story's history before it records
    // anything into it. Without this, an attempt that ran before a restart is
    // invisible to the retry hash, and every alert is re-recorded as new. Called
    // once per story per process; never throws, and cannot gate (see the sink).
    await forgeObserverSink.load({
      storyId: resolvedStory.id,
      processInstanceId: task.processInstanceId,
    })
    observeAttemptBegin(forgeObserverSink, observerAttempt, {
      role: nodeId,
      // The Lead's chosen route rides run.start (an event kind already in the
      // union) rather than a new `lead.decide` kind.
      route: evidence.leadDecision ?? null,
    })
    const attemptStatus: AttemptStatus = /hold/i.test(result.evidence.resultStatus)
      ? 'interrupted'
      : /pass|success|complete/i.test(result.evidence.resultStatus)
        ? 'completed'
        : 'failed'
    observeAttemptEnd(forgeObserverSink, observerAttempt, {
      status: attemptStatus,
      sha: candidateSha,
      storyId: resolvedStory.id,
    })
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
        // OBSERVER (phase 1): record the candidate's commit and scope verdict
        // BEFORE the HOLD below decides, so a denied candidate leaves a trace even
        // though it throws. The sink cannot throw, so this can never change the
        // outcome — and the HOLD below is still the runner's own throw, never an
        // Alert recommendation.
        const observerIdentity = {
          storyId: resolvedStory.id,
          processInstanceId: task.processInstanceId,
          taskId: durableClaim.id,
          nodeId,
          attempt: splitAssignmentContract.identity.attempt,
          worktreePath: cwd,
          baseCommit: workspaces?.baseRef ?? 'origin/main',
        }
        const observed = observeCandidateCommit(forgeObserverSink, observerIdentity, {
          candidateSha: evidence.candidateSha,
          changedFiles,
          contract: splitAssignmentContract,
        })
        drainAlerts(forgeObserverSink, observerIdentity, resolvedStory.id)
        if (observed.violations.length > 0) {
          const reason =
            `candidate ${evidence.candidateSha.slice(0, 12)} touched files outside its assignment ` +
            `(${splitAssignmentContract.identity.owner}): ${observed.violations.join(', ')}`
          observeHold(forgeObserverSink, observerIdentity, {
            reasons: [reason],
            sha: evidence.candidateSha,
          })
          throw new Error(`Forge ${nodeId} HOLD: ${reason}`)
        }
      }
      await recordSplitChildCandidate(durableClaim.id, evidence.candidateSha)
    }

    // ENG-FORGE-OBS-SERIAL-01 — the SERIAL lane (`smith`, `lead_solo_implement`),
    // which is the path that actually runs every story.
    //
    // Same candidate facts the split children get: the commit, and a SCOPE_CHECK
    // against the assignment the Lead accepted for this lane.
    //
    // DOOR 2 — SERIAL SMITH SCOPE (FORGE-SMITH-DOOR-01)
    //
    // The lane that runs every story: it records the candidate's commit and a
    // SCOPE_CHECK against the assignment the Lead accepted, and a violation is a
    // MISS, not a note — the self-heal reprompt names the exact paths and only
    // exhaustion is a HOLD, thrown by the runner below rather than by an Alert.
    // (Until b484301 this lane enforced nothing; door 1 above now guarantees the
    // contract exists, so "no assignment → nothing to enforce" is unreachable here.)
    if (executesLeadWorkOrders && candidateSha) {
      const cwd = workspaces?.worktreesRoot
        ? deriveWorktreePath(workspaces.worktreesRoot, resolvedStory.id, executionId)
        : process.cwd()
      const serialIdentity = {
        storyId: resolvedStory.id,
        processInstanceId: task.processInstanceId,
        taskId: durableClaim.id,
        nodeId,
        attempt: attempt + 1,
        worktreePath: cwd,
        baseCommit: workspaces?.baseRef ?? 'origin/main',
      }
      try {
        const changedFiles = await changedFilesForCandidate({
          cwd,
          baseRef: workspaces?.baseRef ?? 'origin/main',
          candidateSha,
        })
        const observed = observeCandidateCommit(forgeObserverSink, serialIdentity, {
          candidateSha,
          changedFiles,
          contract: serialAssignmentContract,
        })
        drainAlerts(forgeObserverSink, serialIdentity, resolvedStory.id)
        // ENG-FORGE-SMITH-EXIT — the candidate's OWN line, judged against git.
        //
        // The live doors stay the authority (launch = serialLaunchDoor, scope =
        // scopeViolations inside assessSmithExit). This adds only what the live door
        // did not have: a parsed SMITH_CANDIDATE line, a NAMED miss when it is
        // absent, the claimed SHA vs worktree HEAD, the assignment id, and an empty
        // diff. `runnerDiff` is git's answer and always wins over the model's claim.
        if (serialAssignment && serialAssignmentContract) {
          const smithExit = assessSmithExit({
            nodeId,
            assignmentId: serialAssignment.id,
            contract: serialAssignmentContract,
            notes: raw,
            runnerDiff: {
              candidateSha,
              mergeBase: workspaces?.baseRef ?? 'origin/main',
              changedPaths: changedFiles,
            },
          })
          if (!smithExit.ok) serialScopeMiss = smithExit.reasons
        }
        // ENG-FORGE-OBS-SERIAL-01 box 1 — the violation is now a MISS, not a note.
        //
        // The lane used to accept any candidate: declared scope was measured but not
        // enforced, so a Smith that wandered out of its assignment still handed over
        // work (the same missing lock ENG-FORGE-SPLIT-01 found on the split path).
        // The miss rides the existing bounded self-heal, so attempt 2 gets a
        // corrective directive naming the exact paths, and only exhaustion is a real
        // HOLD — thrown by the runner below, never by an Alert recommendation.
        if (observed.violations.length > 0) {
          const owner = serialAssignmentContract?.identity.owner ?? 'the accepted assignment'
          serialScopeMiss = serialScopeMissReasons(observed.violations, owner)
        }
      } catch (error) {
        // FAIL CLOSED. An unreadable diff is not proof that the candidate stayed
        // inside its accepted assignment — it is the ABSENCE of that proof, and the
        // scope lock is an authorization boundary, not a measurement. The split lane
        // already refuses a child whose diff it cannot read (its diff is a join
        // precondition); this lane used to record the gap and advance anyway.
        //
        // The bounded self-heal below is the safety valve, so a transient git failure
        // costs one corrective attempt naming the real reason rather than the run:
        // only exhaustion is a HOLD.
        const detail = String((error as Error)?.message ?? error)
        observeMeasurementGap(forgeObserverSink, serialIdentity, {
          what: 'SERIAL_SCOPE',
          reason: `candidate ${candidateSha.slice(0, 12)} diff unreadable: ${detail}`,
        })
        serialScopeMiss = [
          `candidate ${candidateSha.slice(0, 12)} could not be measured against its assignment ` +
            `(${serialAssignmentContract?.identity.owner ?? 'the accepted assignment'}): ` +
            `its diff is unreadable — ${detail}`,
        ]
      }
    }
    // LEAD routing (Astra handoff): the AI proposes, code accepts or returns
    // corrections — it never silently reroutes. The validated proposal is the
    // single source of engine routing facts, and a malformed/absent proposal is a
    // miss that rides the bounded self-heal path below (never a default route).
    // ENG-FORGE-PHASE-AGENT: for lead_pre the phase agent's collect() already ran
    // the live reviewer (same function, same trusted context) and assigned the
    // routing facts. This is the LEGACY FALLBACK for the case where collect could
    // not supply findings and therefore left evidence untouched — so the
    // trusted-context validation is never skipped, but never runs twice either.
    // The DECISION travels in FIELDS now (migration 170), written by
    // scripts/forge-handoff.mjs. Prefer the recorded contract over parsing the reply:
    // a marker prefix going missing must never cost a routing decision the engine
    // already has in hand — which is exactly what cost us 18 minutes tonight.
    // ONE seat, ONE source. The rows were read before the ports were built, and the
    // phase agent has already resolved and reviewed them through the same helper. This
    // fallback now exists ONLY for a lead_pre whose collect did not run at all — it is
    // guarded on the rejection too, because re-reviewing a refusal is how the runner
    // could ACCEPT what collect had just refused, with different errors.
    const routingReview =
      nodeId === 'lead_pre' && evidence.leadDecision == null && evidence.deliverableRejection == null
        ? resolveLeadProposal({
            contract: recordedContract,
            plan: recordedPlan,
            context: leadRoutingContext,
          })
        : null
    if (routingReview?.ok) {
      Object.assign(evidence, leadRoutingFacts(routingReview))
    }
    // A Lead who DECIDES HOLD is a valid routing outcome (`reviewLeadProposal`
    // returns ok for it), so nothing throws and the engine advances on the
    // decision. Before this, the trace showed the lane completing with no record
    // of the hold it had just decided — a Lead PRE HOLD with no HOLD event.
    // Recorded only: the engine still owns what happens next.
    // Either source may have decided HOLD: the phase agent's collect() (normal
    // path) or the legacy fallback above. Both are recorded the same way.
    const leadHoldReason =
      routingReview?.ok && routingReview.proposal.decision === 'HOLD'
        ? routingReview.proposal.reason || routingReview.proposal.sizeReason || 'no reason given'
        : nodeId === 'lead_pre' && evidence.leadDecision === 'HOLD'
          ? String(
              (evidence.leadRouting as { reason?: string; sizeReason?: string } | undefined)?.reason ||
                (evidence.leadRouting as { sizeReason?: string } | undefined)?.sizeReason ||
                'no reason given',
            )
          : null
    if (leadHoldReason) {
      observeRouteHold(forgeObserverSink, observerAttempt, leadHoldReason)
      drainAlerts(forgeObserverSink, observerAttempt, resolvedStory.id)
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
      // ENG-FORGE-ARCHITECT-BRIEF-01 (slice 1) — the Architect brief is a CONTRACT,
      // enforced at the ARCHITECT boundary so a bad brief names the Architect, not
      // LEAD. Node-scoped (not lane-scoped): `research_architect` shares lane
      // 'architect' but owes a disposition, not findings. This cannot add HOLD risk:
      // an empty/invalid brief already HOLDs one lane later — it just HOLDs the wrong
      // role with a message the author could not act on. Errors ride the same bounded
      // self-heal path as LEAD routing, so attempt 2 can fix them.
      if (nodeId === 'architect' || nodeId === 'repair_architect') {
        // TWO contracts, ONE boundary. A FORGE_ARCHITECT_HANDOFF reply is assessed
        // by the phase agent itself (assessArchitectHandoff, which additionally
        // checks that every named seam EXISTS on the pinned baseRef); its verdict
        // already rides `deliverableRejection` into the gate above, and the phase
        // agent now also records a rejection when no plan came back at all.
        //
        // The legacy assessment therefore runs ONLY when the phase agent recorded
        // no rejection, which in practice means the reply really used the legacy
        // marker — that must keep passing. Running it otherwise was worse than
        // redundant: it put a PROSE reason inside `missing` (whose documented
        // invariant is a stable-kind list, with prose kept in the rejection
        // sidecar) and it asked the model for FORGE_FINDINGS_JSON — the FALLBACK
        // marker — directly contradicting the FORGE_ARCHITECT_HANDOFF marker the
        // directive names. The retry is the one attempt the model gets, and it was
        // being taught the wrong contract by it.
        const brief =
          evidence.deliverableRejection != null || parseArchitectHandoff(raw) !== null
            ? { verdict: 'OK' as const, reasons: [] as string[] }
            : assessArchitectBrief(raw)
        if (brief.verdict === 'HOLD') {
          missing.push(...brief.reasons.map((reason) => `architect-brief:${reason}`))
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
        // durable observability of its verdict, not a second gate. (The redundant
        // nested `if (nodeId === 'lead_pre')` was removed 2026-09-13.)
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
      miss.push(...missing)
    }

    // The serial lane's scope violation joins the same HOLD path as every other gate:
    // reprompt with the named paths, or throw on the final attempt.
    //
    // PUSHED OUTSIDE the `enforceDeliverables` block on purpose. That flag is named for
    // DELIVERABLES, but this push used to sit inside it — so a run started with
    // FORGE_ENFORCE_DELIVERABLES=0 also switched off the Smith authorization boundary,
    // and a candidate that touched files outside its accepted assignment advanced
    // unchallenged. A feature flag must not be able to disarm an authorization check.
    miss.push(...serialScopeMiss)

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

    // The REASONS a role's own gate rejected its output, carried as a SIDECAR to
    // `missing` — never inside it. `missing` stays the stable-kind list that the
    // retry hash and the alert rule key on, so adding prose here cannot make an
    // unchanged retry look like a changed one.
    const rejectionReasons = (evidence.deliverableRejection ?? '')
      .split('; ')
      .map((reason) => reason.trim())
      .filter(Boolean)

    // Bounded self-heal: an otherwise-successful run that missed a deliverable or
    // routing decision is re-run with a corrective directive naming what was
    // missing. Only when the reprompt budget is exhausted do we throw the HOLD.
    if (attempt + 1 < totalAttempts) {
      correctiveNote = buildSelfHealDirectiveWithReasons(
        nodeId,
        miss,
        leadRoutingGovernsPre && leadRoutingContext
          ? buildLeadRoutingDirective(leadRoutingContext) +
            '\n' +
            seamGroupHint(
              leadRoutingContext.findings.map((f) => ({
                id: f.id,
                required: Boolean(f.required),
                summary: '',
                preconditions: [],
                scope: f.seams ?? [],
                postconditions: [],
                classes: [],
                risks: [],
                hint: f.hint,
              })),
            ).text
          : plan.lane === 'smith' && serialAssignment
            ? buildSmithDirective(
                assignmentFromLead({
                  id: serialAssignment.id,
                  findingIds: serialAssignment.findingIds ?? [],
                  chunks: (serialAssignment.plan?.chunks ?? []).map((c, i) => ({
                    id: c.id ?? i + 1,
                    scope: c.surface ?? [],
                    proof: c.proof ?? '',
                  })),
                }),
              )
            : plan.evidenceInstruction,
        rejectionReasons.length ? rejectionReasons : undefined,
        priorReply,
      )
      continue
    }
    // The runner's OWN throw is the authority here — never an Alert. Recording it
    // is what the serial lane never did: a Smith / Lead / QA HOLD left no durable
    // trace at all. `miss` doubles as the retry hash, so a second attempt that
    // changed nothing becomes visible as RETRY_UNCHANGED_INPUT instead of looking
    // like an unrelated retry.
    observeHold(forgeObserverSink, observerAttempt, {
      reasons: miss,
      sha: candidateSha,
      missReasons: miss,
    })
    // ...and record it WHERE THE HUMAN READS IT. The observer sink keeps the event
    // history; the run detail carries the last fact that ended the run, which is
    // what /portal/tech/runs shows. A HOLD whose reason exists only in a log file
    // is the write-only durable record this lane already paid for once.
    const holdRunId = finishedItem?.storyRunId ?? null
    if (holdRunId) {
      await appendForgeRunDetail(
        holdRunId,
        `Forge ${nodeId} HOLD code=DELIVERABLE_REJECTED (after ${totalAttempts} attempt(s)): ` +
          `role did not deliver ${miss.join(', ')}` +
          (rejectionReasons.length ? ` — refused because: ${rejectionReasons.join('; ')}` : ''),
      ).catch(() => {
        /* run-detail append is observer-only; the HOLD throw below stands */
      })
    }
    throw new Error(
      `Forge ${nodeId} HOLD (after ${totalAttempts} attempt(s)): role did not deliver ${miss.join(', ')}`,
    )
    }
    // Unreachable: the loop always returns (pass) or throws (final HOLD).
    throw new Error(`Forge ${nodeId} exhausted reprompt attempts without a verdict`)
  }
}
