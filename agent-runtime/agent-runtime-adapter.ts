// ---------------------------------------------------------------------------
// Abstract AgentRuntimeAdapter — the CulebraLuxe-owned execution contract.
//
// PUBLIC / STABLE operations (SHARED lifecycle behavior lives here):
//   execute, status, pause, resume, cancel, result
//
// VENDOR-SPECIFIC behavior lives ONLY in the protected abstract hooks:
//   startExternal, statusExternal, pauseExternal, resumeExternal,
//   cancelExternal, resultExternal
//
// The base class OWNS common orchestration + persistence through injected
// repositories/services — concrete adapters never emit raw SQL and never
// re-implement the shared lifecycle. No vendor nouns may appear anywhere in
// this contract.
// ---------------------------------------------------------------------------

import { captureServerLog } from '../lib/server-error-capture'
import { recordToolArtifact } from '../db/forge-artifact'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import type {
  AgentRunRepository,
  AgentWorkRepository,
} from './repositories'
import type { AgentCapability } from './capabilities'
import { withWorkspaceEvidence } from './candidate-assay-handoff'
import {
  parseTestMode,
  resolveTestModeFromInstructions,
  withTestModeDirective,
} from './test-mode'

/** Minimum wall-clock interval between heartbeat progress writes (ms).
 * Bounds the DB write cadence of the status-poll loop while keeping liveness
 * fresh well inside the stale-recovery threshold. Meaningful note/step changes
 * persist immediately regardless of this interval. */
export const HEARTBEAT_MIN_INTERVAL_MS = 5000

/**
 * How long the vendor-status poll loop waits between checks.
 *
 * The loop used to run with NO delay, calling `statusExternal` back-to-back for the entire life of a
 * run. When the vendor check is cheap (a local status file) that is a hot loop, and it is what turned
 * a single stale work item into 45 captured errors in 14 seconds.
 */
export const PROGRESS_POLL_INTERVAL_MS = 1000

/** Default single-run wall-clock hard limit. A wandering role run dies here,
 * never burning credits indefinitely. Override via FORGE_RUN_MAX_MS. */
export const DEFAULT_RUN_WALL_CLOCK_MS = 45 * 60_000

/** Parse the per-run wall-clock budget (ms) from env, falling back to default. */
export function runWallClockBudgetMs(env: NodeJS.ProcessEnv = process.env): number {
  const raw = (env.FORGE_RUN_MAX_MS ?? '').trim()
  const n = Number(raw)
  return Number.isFinite(n) && n > 0 ? n : DEFAULT_RUN_WALL_CLOCK_MS
}

export type AdapterLifecycle =
  | 'not_started'
  | 'starting'
  | 'running'
  | 'paused'
  | 'success'
  | 'failed'
  | 'cancelled'

export type AdapterStatus = {
  lifecycle: AdapterLifecycle
  externalRunId: string | null
  detail: Record<string, unknown>
}

export interface AgentRuntimeAdapterDeps {
  work: AgentWorkRepository
  runs: AgentRunRepository
  /** Clock injection for deterministic tests. */
  now?: () => Date
}

/** External start outcome (opaque correlation id only). */
export type ExternalStartResult = {
  externalRunId: string
}

export type ExternalStatusResult = {
  lifecycle: AdapterLifecycle
  detail?: Record<string, unknown>
}

export abstract class AgentRuntimeAdapter {
  /** Stable adapter identity (e.g. 'tunit', 'deepseek-harness', 'local-mac'). */
  abstract readonly runtimeAdapterId: string

  /** Capabilities this adapter can satisfy. */
  abstract readonly capabilities: AgentCapability[]

  /** Opaque external run/session correlation for the current attempt. */
  protected externalRunId: string | null = null

  /** Optional external error detail captured by the vendor hooks. */
  protected externalErrorText: string | null = null

  /** Frozen Story Run execution view of the most recent execute. */
  protected currentStory: AgentExecutionContext['story'] | null = null

  constructor(protected readonly deps: AgentRuntimeAdapterDeps) {}

  // -------------------------------------------------------------------------
  // PUBLIC / STABLE OPERATIONS (shared orchestration — do not override)
  // -------------------------------------------------------------------------

  /**
   * Execute one AgentWorkCommand as one logical run. SHARED orchestration:
   *   1. terminal/idempotency guard using the PERSISTED command state
   *   2. claim-if-Ready (atomic, single-worker-guarded)
   *   3. begin the authoritative Story Board run and freeze its contract
   *   4. replace mutable parent Story context with that frozen Run context
   *   5. startExternal -> opaque externalRunId persisted on the work item
   *   6. heartbeat/progress loop over statusExternal until terminal
   *   7. normalize result: fail / cancel / finish according to terminal state
   * Concrete adapters provide ONLY the vendor hooks.
   */
  async execute(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence> {
    this.assertNotTerminal(command)

    // Re-read the durable command so the terminal/active decision uses the
    // PERSISTED state, not a possibly-stale in-memory copy.
    const persisted = await this.deps.work.get(command.workItemId)
    if (!persisted) {
      throw new Error(`command ${command.workItemId} no longer exists`)
    }
    if (persisted.state === 'Done' || persisted.state === 'Error' || persisted.state === 'Cancelled') {
      throw new Error(`command ${command.workItemId} is already terminal (${persisted.state})`)
    }
    command = { ...command, state: persisted.state as AgentWorkCommand['state'] }

    // Invoker-style claim: if the durable command is still Ready, claim it
    // (atomic, single-worker-guarded); if already Claimed, proceed.
    if (command.state === 'Ready') {
      const claimed = await this.deps.work.claimSpecific(command.workItemId, this.runtimeAdapterId)
      if (!claimed) {
        throw new Error(
          `command ${command.workItemId} could not be claimed (another item active or state changed)`,
        )
      }
      command = { ...command, state: 'Claimed', claimedBy: this.runtimeAdapterId }
    }

    // This is the V6 authority boundary. beginRun snapshots the parent Story
    // into storyboard_story_run and returns a Story-shaped view reconstructed
    // from that frozen Run. From this line onward no lane executes from mutable
    // parent contract fields.
    const begun = await this.deps.work.beginRun(command.workItemId)
    const runId = begun.workItem.storyRunId!
    const executionStory = begun.story as AgentExecutionContext['story']
    command = { ...command, storyRunId: runId, state: 'Running' }

    // Test-mode is part of the frozen Run contract. Rewrite any older envelope
    // directive to the Run snapshot before Smith/Architect adapters see it.
    const frozenTestMode = parseTestMode(executionStory.testMode)
    if (frozenTestMode) {
      const parsed = resolveTestModeFromInstructions(command.specialInstructions, null)
      command = {
        ...command,
        specialInstructions: withTestModeDirective(
          parsed.instructions ?? '',
          frozenTestMode,
        ),
      }
    }

    this.currentStory = executionStory
    const ctxWithRun: AgentExecutionContext = {
      ...context,
      command,
      story: executionStory,
      storyRunId: runId,
    }

    const started = await this.startExternal(ctxWithRun)
    this.externalRunId = started.externalRunId
    const runStartedAtMs = Date.now()
    const runBudgetMs = runWallClockBudgetMs()
    await this.deps.work.setRuntime(command.workItemId, {
      runtimeAdapter: this.runtimeAdapterId,
      externalRunId: this.externalRunId,
    })

    // Persist running + heartbeat.
    await this.deps.work.progress(command.workItemId, {
      step: 'executing',
      completion: 10,
      note: `run started via ${this.runtimeAdapterId} (external ${this.externalRunId})`,
    })

    // Heartbeat/progress loop until the vendor runtime reaches a terminal
    // lifecycle. Every pass refreshes liveness (heartbeat) via the work repo,
    // but identical heartbeat content is coalesced (HEARTBEAT IS STATE, NOT
    // HISTORY): the durable layer dedupes unchanged notes, and this loop
    // throttles the DB write cadence so a fast vendor status poll cannot become
    // a write amplifier. Meaningful note/step changes still persist immediately.
    let status = await this.statusExternal(command, ctxWithRun)
    let lastHeartbeatAt = 0
    // ONCE THE WORK ITEM IS NO LONGER RUNNING, PROGRESS IS POINTLESS - SO STOP TRYING.
    //
    // `progress()` refuses with a conflict when the row is no longer Running ("Work item ... is not
    // Running; progress requires an active run."). The loop below kept calling it anyway and captured
    // the refusal on EVERY poll whose content had changed, which measured as 45 error rows in 14
    // SECONDS for a single stale work item - a self-inflicted flood of the error store we rely on to
    // diagnose everything else. A conflict is a terminal fact about this run, not a transient one, so
    // it is recorded ONCE and progress is disabled for the rest of the loop; the run still finishes and
    // its real evidence is verified independently.
    let progressDisabled = false
    while (status.lifecycle === 'running') {
      const prog = this.progressFromStatus(status, command)
      const now = Date.now()
      // Wall-clock hard limit: a wandering run is cancelled (SIGTERM) so the
      // terminalize path records it as interrupted -> engine HOLD, never success.
      if (now - runStartedAtMs >= runBudgetMs) {
        this.externalErrorText = `RUN_BUDGET: exceeded ${Math.round(runBudgetMs / 60000)}m wall-clock hard limit; run cancelled.`
        await this.cancelExternal(command, ctxWithRun)
      }
      const changed = Boolean(prog?.note) || Boolean(prog?.step) || prog?.completion != null
      if (!progressDisabled && (changed || now - lastHeartbeatAt >= HEARTBEAT_MIN_INTERVAL_MS)) {
        lastHeartbeatAt = now
        try {
          await this.deps.work.progress(command.workItemId, {
            step: prog?.step,
            completion: prog?.completion,
            note: prog?.note,
          })
        } catch (err) {
          // ENG-FORGE-SPLIT-01: progress is OBSERVABILITY (heartbeat + milestones),
          // not control flow. Under concurrent siblings a transient failure here
          // aborted a healthy child mid-run. Record it durably and keep the run
          // going — the child's real evidence is verified independently.
          const message = String((err as { message?: string })?.message ?? err)
          progressDisabled = /is not Running|has no story run/i.test(message)
          captureServerLog('warn', 'forge.adapter.progress', message, {
            route: 'forge:role-progress',
          })
        }
      }
      // POLL PACING. This loop had no delay at all: `statusExternal` was called back-to-back for the
      // whole life of a run, which spins whenever the vendor check is cheap (a local status file),
      // and it fed the flood above. One second between polls is still immediate to a human watching
      // the screen and costs nothing next to the model call each turn is waiting on.
      await new Promise((resolve) => setTimeout(resolve, PROGRESS_POLL_INTERVAL_MS))
      status = await this.statusExternal(command, ctxWithRun)
    }

    if (status.lifecycle === 'failed') {
      await this.deps.work.fail(
        command.workItemId,
        this.externalErrorText ?? 'runtime failed',
      )
      const item = await this.deps.work.get(command.workItemId)
      return this.normalizeEvidenceFromRun(item!.storyRunId!, command)
    }

    if (status.lifecycle === 'cancelled') {
      await this.deps.work.cancel(command.workItemId, this.externalErrorText ?? 'runtime cancelled')
      const item = await this.deps.work.get(command.workItemId)
      return this.normalizeEvidenceFromRun(item!.storyRunId!, command)
    }

    // Success path.
    const result = await this.resultExternal(command, ctxWithRun)
    // ENG-FORGE-V4-10C: every run that executed inside an isolated workspace
    // records its base commit as machine-scannable evidence. V6 also carries
    // structured Assay evidence beside this human-readable compatibility line.
    const notes = withWorkspaceEvidence(ctxWithRun.executionWorkspace, result!.notes)
    const finished = await this.deps.work.finish(command.workItemId, {
      resultStatus: result!.resultStatus,
      completion: result!.completion,
      notes,
      commitHash: result!.commitHash,
      testsSummary: result!.testsSummary,
      assayEvidence: result!.assayEvidence ?? null,
      modelUsed: result!.modelUsed ?? null,
      // The adapter measured this role's spend against the session it pinned; carry it to
      // the run row. Absent when the harness store could not be read (unmeasured, honestly).
      harnessUsage: result!.harnessUsage ?? null,
    })
    const evidence = {
      ...this.normalizeEvidence(finished.run as any, command),
      assayEvidence: result!.assayEvidence ?? null,
    }
    // The completion path is the one every ordinary lane takes, so this is the call that makes an
    // architect, lead or smith run visible in forge_tool_artifact (Captain, 2026-09-16).
    await this.recordRunArtifact(evidence, command, (finished.run as { id?: string } | null)?.id ?? null)
    return evidence
  }

  /** Query runtime status from persisted canonical state + optional vendor detail. */
  async status(command: AgentWorkCommand): Promise<AdapterStatus> {
    const item = await this.deps.work.get(command.workItemId)
    const state = item?.state ?? command.state
    const runId = this.externalRunId ?? item?.externalRunId ?? null
    const detail: Record<string, unknown> = { externalRunId: runId }
    if (state === 'Done') return { lifecycle: 'success', externalRunId: runId, detail }
    if (state === 'Error') return { lifecycle: 'failed', externalRunId: runId, detail }
    if (state === 'Cancelled') return { lifecycle: 'cancelled', externalRunId: runId, detail }
    if (state === 'Paused') return { lifecycle: 'paused', externalRunId: runId, detail }
    if (state === 'Running' || state === 'Claimed') {
      return { lifecycle: 'running', externalRunId: runId, detail }
    }
    return { lifecycle: 'not_started', externalRunId: null, detail }
  }

  /** Pause an active run: vendor hook + durable heartbeat (assignment preserved). */
  async pause(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    await this.pauseExternal(command, this.contextWithRun(command))
    await this.deps.work.progress(command.workItemId, {
      step: 'paused',
      note: `${this.runtimeAdapterId} runtime paused`,
    })
  }

  /** Resume a paused run: vendor hook + durable heartbeat (same logical attempt). */
  async resume(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    await this.resumeExternal(command, this.contextWithRun(command))
    await this.deps.work.progress(command.workItemId, {
      step: 'executing',
      note: `${this.runtimeAdapterId} runtime resumed`,
    })
  }

  /** Cancel an active run: vendor hook + canonical cancellation (never success). */
  async cancel(command: AgentWorkCommand): Promise<void> {
    this.assertNotTerminal(command)
    await this.cancelExternal(command, this.contextWithRun(command))
    await this.deps.work.cancel(command.workItemId, `${this.runtimeAdapterId} runtime cancelled`)
  }

  /**
   * Retrieve normalized evidence. A result may only be treated as successful
   * after the work item is terminal; otherwise the run is not complete.
   */
  async result(command: AgentWorkCommand): Promise<AgentRunEvidence | null> {
    const item = await this.deps.work.get(command.workItemId)
    if (!item?.storyRunId) return null
    const runs = await this.deps.runs.listForStory(item.storyId)
    const run = runs.find((r) => r.id === item.storyRunId) ?? null
    if (!run) return null
    if (item.state !== 'Done' && item.state !== 'Error' && item.state !== 'Cancelled') {
      return null
    }
    return this.normalizeEvidence(run as any, command)
  }

  // -------------------------------------------------------------------------
  // PROTECTED VENDOR HOOKS — the ONLY place concrete adapters translate to a
  // specific harness/model. No shared lifecycle logic belongs here.
  // -------------------------------------------------------------------------

  protected abstract startExternal(
    context: AgentExecutionContext,
  ): Promise<ExternalStartResult>

  protected abstract statusExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<ExternalStatusResult>

  protected abstract pauseExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<void>

  protected abstract resumeExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<void>

  protected abstract cancelExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<void>

  protected abstract resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null>

  // -------------------------------------------------------------------------
  // SHARED helpers
  // -------------------------------------------------------------------------

  /** Shared terminal-state guard. */
  protected assertNotTerminal(command: AgentWorkCommand): void {
    if (command.state === 'Done' || command.state === 'Error' || command.state === 'Cancelled') {
      throw new Error(
        `command ${command.workItemId} is already terminal (${command.state})`,
      )
    }
  }

  /** Optional per-status progress projection (concrete adapters override to
   * expose factual step/completion/note from a status poll). */
  protected progressFromStatus(
    _status: ExternalStatusResult,
    _command: AgentWorkCommand,
  ): { step?: string; completion?: number; note?: string } | null {
    return null
  }

  private contextWithRun(command: AgentWorkCommand): AgentExecutionContext {
    // Pause/resume/cancel hooks retain the same frozen Run-bound Story view.
    return {
      command,
      story: this.currentStory as never,
      policy: { allowCommit: false, allowDevDbWrite: false, allowControlPlaneWrite: true },
      capabilities: this.capabilities,
      storyRunId: command.storyRunId ?? '',
    }
  }

  protected normalizeEvidence(
    run: {
      id: string
      storyId: string
      startedAt: string
      endedAt: string | null
      resultStatus: string | null
      completion: number | null
      notes: string | null
      testsSummary: string | null
      commitHash: string | null
      executionEnvironment?: string | null
    },
    command: AgentWorkCommand,
  ): AgentRunEvidence {
    return {
      resultStatus: run.resultStatus ?? 'Failed',
      completion: run.completion ?? 0,
      notes: run.notes ?? '',
      testsSummary: run.testsSummary ?? null,
      commitHash: run.commitHash ?? null,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: command.modelProfile,
      externalRunId: this.externalRunId,
      executionEnvironment: run.executionEnvironment ?? command.executionEnvironment ?? null,
      startedAt: run.startedAt,
      endedAt: run.endedAt,
    }
  }

  /**
   * Write this lane's artifact to Neon: the verdict and the work it did, keyed to the story and the run.
   *
   * Called from BOTH ways a run can finish — the normal completion at the end of `execute` and the
   * terminal/result-lookup branch — because they normalize evidence through two different methods, and
   * hooking one of them left every ordinary lane silent (2026-09-16: the lead lane finished with no row).
   */
  private async recordRunArtifact(
    evidence: AgentRunEvidence,
    command: AgentWorkCommand,
    storyRunId: string | null,
  ): Promise<void> {
    try {
      await recordToolArtifact({
        storyId: command.storyId,
        storyRunId,
        tool: this.runtimeAdapterId,
        kind: 'run-verdict',
        verdict: evidence.resultStatus,
        summary: evidence.testsSummary ?? null,
        sha: evidence.commitHash ?? null,
        detail: {
          notes: evidence.notes,
          testsSummary: evidence.testsSummary ?? null,
          completion: evidence.completion,
          modelProfile: command.modelProfile ?? null,
          externalRunId: evidence.externalRunId ?? null,
          executionEnvironment: evidence.executionEnvironment ?? null,
          startedAt: evidence.startedAt ?? null,
          endedAt: evidence.endedAt ?? null,
        },
      })
    } catch (err) {
      captureServerLog(
        'warn',
        'agent-runtime.artifact-write',
        `${this.runtimeAdapterId} ${storyRunId ?? 'no-run'}: ${(err as Error).message}`,
      )
      console.warn(
        `artifact write skipped (${this.runtimeAdapterId} run ${storyRunId ?? 'unknown'}): ${(err as Error).message}`,
      )
    }
  }

  /**
   * EVERY AGENT WRITES ITS ARTIFACT TO NEON (Captain, 2026-09-16). This is the base class's single funnel: every
   * adapter returns its evidence through here, so ONE write puts a verdict row in `forge_tool_artifact` for every
   * lane — by inheritance, not by each lane remembering to do it. The verdict, summary and sha come straight from
   * the run row; nothing is inferred. A failed write warns and never fails the run, because a missing artifact
   * row must be visible rather than crash the lane that produced it.
   */
  protected async normalizeEvidenceFromRun(
    runId: string,
    command: AgentWorkCommand,
  ): Promise<AgentRunEvidence> {
    const evidence = await this.normalizeEvidenceFromRunInner(runId, command)
    await this.recordRunArtifact(evidence, command, runId)
    return evidence
  }

  private async normalizeEvidenceFromRunInner(
    runId: string,
    command: AgentWorkCommand,
  ): Promise<AgentRunEvidence> {
    const runs = await this.deps.runs.listForStory(command.storyId)
    const run = runs.find((r) => r.id === runId)
    if (!run) {
      return {
        resultStatus: 'Failed',
        completion: 0,
        notes: 'run not found after terminal transition',
        testsSummary: null,
        commitHash: null,
        runtimeAdapter: this.runtimeAdapterId,
        modelProfile: command.modelProfile,
        externalRunId: this.externalRunId,
        startedAt: new Date().toISOString(),
        endedAt: new Date().toISOString(),
      }
    }
    return this.normalizeEvidence(run as any, command)
  }
}
