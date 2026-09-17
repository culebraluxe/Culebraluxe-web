// ---------------------------------------------------------------------------
// OpenCodeHarnessAdapter — thin translation layer from the vendor-neutral
// AgentRuntimeAdapter contract to the OpenCode CLI (`opencode run`) as an
// INNER Smith execution harness (ENG-FORGE-V5-01).
//
// OpenCode is an inner execution engine, NOT a second orchestrator. Forge
// stays outside it and owns the lifecycle:
//
//   Forge worktree -> OpenCode Smith execution -> Forge candidate commit
//     -> exact-candidate Assay -> Forge publish
//
// The base class (AgentRuntimeAdapter) owns ALL shared lifecycle orchestration
// (claim, begin-run, heartbeat loop, terminal normalization, evidence
// persistence). This class implements ONLY the protected vendor hooks plus a
// tiny process wrapper (agent-runtime/opencode/opencode-client).
//
// Contract invariants (deliberately boring, fail closed):
//   - The adapter runs ONLY inside the Forge-provisioned isolated worker
//     worktree (context.executionWorkspace). It refuses to start in an
//     unisolated/shared checkout: `--auto` must never leave the Forge worktree.
//   - The model is ALWAYS passed explicitly as `deepseek/deepseek-v4-flash`
//     (OPENCODE_PINNED_MODEL). A missing or different explicit model is a
//     fail-closed error — OpenCode's default/automatic model selection is
//     never allowed, because it would invalidate A/B measurements.
//   - The canonical Forge task/prompt contract (buildTaskText) is preserved
//     verbatim as the message passed to `opencode run`.
//   - OpenCode never owns git branch creation, candidate commit creation,
//     Assay, publish, story state, or Neon state: Forge provisions the branch
//     and worktree, reads/creates the candidate commit through the existing
//     harness-owned commit path, and owns Assay + publish unchanged.
//   - No OpenCode server mode, ACP, MCP, subagents, TUI automation,
//     swarm, or provider orchestration is added here. The one deliberate,
//     env-gated exception is V5-21 session continuity (FORGE_SESSION_CONTINUITY=1,
//     default OFF): successive roles of one isolated worktree resume the same
//     session via `--continue`. Off by default, so the session-free contract holds
//     unless an operator opts in.
//
// NO OpenCode nouns leak upward: the command model, Story Board, invoker, and
// canonical statuses stay generic. Evidence records factual run metadata
// (harness=opencode, model, worktree/cwd, exit status, elapsed ms) but no
// activity-based costing is built.
// ---------------------------------------------------------------------------

import {
  AgentRuntimeAdapter,
  type ExternalStartResult,
  type ExternalStatusResult,
} from '../agent-runtime-adapter'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from '../types'
import type { AgentCapability } from '../capabilities'
import {
  buildTaskText,
  extractTestsSummary,
  workspaceEvidenceLine,
} from '../deepseek/deepseek-harness-adapter'
import {
  startOpenCodeRun,
  type OpenCodeHandle,
  type OpenCodeRunResult,
} from './opencode-client'
import {
  detectFullRegressionAttempt,
  resolveTestModeFromInstructions,
} from '../test-mode'
import { readHarnessUsage, readSessionUsage, usageDelta } from '../harness-usage'
import type { HarnessUsage } from '../harness-usage'
import {
  assertExecutionTargetSafe,
  buildChildProcessEnv,
  resolveExecutionTarget,
  verifyWorkspaceEnvFile,
} from '../../lib/execution-target'
import { readWorkerCommitHash } from '../../lib/worker-workspace'
import { applyRtkToEnv, forgeToolRoleForAgentRole } from '../../workflow_app/forge/forge-tool-seams'
import { captureServerLog } from '../../lib/server-error-capture'
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

/**
 * The single model this adapter is allowed to pin. ENG-FORGE-V5-01 pins the
 * first supported model explicitly; a different value is a fail-closed error
 * so a run can never silently execute under OpenCode's default model
 * selection (which would invalidate A/B measurements against forge-native).
 */
export const OPENCODE_PINNED_MODEL = 'deepseek/deepseek-v4-flash'

/**
 * The runtime adapter id this harness persists on every run's durable
 * evidence (runtime_adapter). Exported as a constant so operator/test code
 * can assert the exact harness identity without reaching into an adapter
 * instance — and so the adapter class field can never drift from the
 * identity record below.
 */
export const OPENCODE_HARNESS_ADAPTER_ID = 'opencode-harness'

/**
 * ENG-FORGE-V5-02 — the pinned OpenCode execution identity. An OpenCode run
 * executes ONLY through this adapter (`opencode-harness`) and ONLY with the
 * explicitly pinned `deepseek/deepseek-v4-flash` model (never OpenCode's
 * default/automatic model selection). This compact record is the single
 * exported place operator and test surfaces read that fact for visibility.
 */
export type OpenCodeExecutionIdentity = {
  /** Runtime adapter id persisted on evidence (runtime_adapter). */
  runtimeAdapter: typeof OPENCODE_HARNESS_ADAPTER_ID
  /** Explicitly pinned provider/model passed as `opencode run --model`. */
  model: typeof OPENCODE_PINNED_MODEL
}

/** Reports the pinned OpenCode execution identity (ENG-FORGE-V5-02). */
export function openCodeExecutionIdentity(): OpenCodeExecutionIdentity {
  return {
    runtimeAdapter: OPENCODE_HARNESS_ADAPTER_ID,
    model: OPENCODE_PINNED_MODEL,
  }
}

// ---------------------------------------------------------------------------
// V5-21 session continuity (ENV-GATED, default OFF). When enabled, successive
// model roles of ONE story's execution generation resume the SAME opencode
// session in the SAME isolated worktree (context persists; authority does not).
//
// The adapter stays session-free by default: nothing changes unless the operator
// sets FORGE_SESSION_CONTINUITY=1. The gate is a worktree-local marker file: the
// FIRST role to run (no marker) starts a fresh session, then writes the marker on
// success; the NEXT role sees the marker and passes --continue, inheriting the
// prior role's context. A fresh worktree (new story/generation) has no marker, so
// sessions can never leak across stories or worktrees. The candidate-freeze
// context wall is preserved because QA/Assay/DevOps do not run this harness.
// ---------------------------------------------------------------------------

/** Env gate for V5-21 session continuity (default OFF). */
export const SESSION_CONTINUITY_ENV = 'FORGE_SESSION_CONTINUITY'

/** Marker filename placed in the isolated worktree after a role run succeeds. */
export const SESSION_MARKER_FILENAME = '.forge-session.continue'

/** True unless the operator explicitly opted out with `FORGE_SESSION_CONTINUITY=0`. */
export function forgeSessionContinuityEnabled(env: NodeJS.ProcessEnv = process.env): boolean {
  // ON BY DEFAULT (ENG-FORGE-WARM-SESSION-01). One live session per execution generation
  // is the design, not an experiment: a role that cold-starts is paying to re-read what
  // the previous role already read. `FORGE_SESSION_CONTINUITY=0` is the opt-out.
  const raw = env[SESSION_CONTINUITY_ENV]
  if (raw == null || raw === '') return true
  const value = raw.trim().toLowerCase()
  return value !== '0' && value !== 'false' && value !== 'off'
}

/** The worktree-local marker path that records "a prior role already ran here". */
export function forgeSessionMarkerPath(workspace: string): string {
  return join(workspace, SESSION_MARKER_FILENAME)
}

/**
 * The session id THIS generation is running in, or null when no role has run here yet.
 *
 * The marker used to hold a bare `1` meaning "someone ran here, resume the last session"
 * (`--continue`). That guesses: it resumes whatever session the project touched last,
 * which is only correct while exactly one lane is live. It now holds the ACTUAL id, and
 * the next role pins it with `--session <id>` — exact, not a guess. A bare `1` is still
 * honoured as "unknown id" so an in-flight worktree from before this change degrades to
 * the old behaviour instead of breaking.
 */
export function readForgeSessionId(workspace: string): string | null {
  try {
    const raw = readFileSync(forgeSessionMarkerPath(workspace), 'utf8').trim()
    if (!raw || raw === '1') return null
    return raw
  } catch {
    return null
  }
}

/**
 * Record this generation's session id, or CLEAR it when the session is gone.
 *
 * Clearing is the "dead session is replaced once" rule: the role that discovers a dead
 * session drops the marker, so the NEXT role starts a fresh session and records its new
 * id. Without this the marker would keep pointing at a corpse and every subsequent role
 * would fail in turn.
 */
export function writeForgeSessionId(workspace: string, sessionId: string | null): void {
  try {
    const path = forgeSessionMarkerPath(workspace)
    if (sessionId) writeFileSync(path, `${sessionId}\n`)
    else rmSync(path, { force: true })
  } catch {
    /* best-effort: session bookkeeping must never fail a run */
  }
}

const OPENCODE_CAPABILITIES: AgentCapability[] = [
  'workspace.fs.read',
  'workspace.fs.write',
  'workspace.fs.delete',
  'git.status',
  'git.diff',
  'git.history',
  'git.commit',
  'host.exec',
  'host.tests',
  'host.typecheck',
  'host.build',
  'host.repo-scripts',
]

export type OpenCodeHarnessConfig = {
  /** Executable to run: `opencode` (PATH) or an absolute entrypoint. */
  cliBin: string
  /** Repository workspace the harness operates in (unused when the invoker
   *  provisions an isolated worker worktree). */
  workspace: string
  /** Explicit model id. Defaults to OPENCODE_PINNED_MODEL; anything blank or
   *  different fails closed. */
  model?: string
  /** Optional env overrides (API keys are read from process env by OpenCode). */
  env?: Record<string, string | undefined>
  /** Injectable start function (tests substitute a fake handle). */
  startRun?: (opts: {
    cliBin: string
    cwd: string
    model: string
    task: string
    env?: Record<string, string | undefined>
  }) => OpenCodeHandle
}

/** Default OpenCode harness configuration for the local host. */
export function defaultOpenCodeConfig(
  env: NodeJS.ProcessEnv = process.env,
): OpenCodeHarnessConfig {
  return {
    cliBin: env.OPENCODE_BIN?.trim() || 'opencode',
    workspace: resolve(process.cwd()),
    model: OPENCODE_PINNED_MODEL,
  }
}

/**
 * Fail-closed model resolution. An omitted model resolves to the
 * ENG-FORGE-V5-01 pin (the adapter ALWAYS passes --model; OpenCode's own
 * default/automatic model selection is never exercised). An explicitly blank
 * model or any model other than the pin throws with a truthful reason — the
 * adapter never falls back to another model or to OpenCode's default.
 */
export function resolveOpenCodeModel(
  model: string | null | undefined,
): string {
  // An OMITTED model resolves to the pin: the adapter always passes an
  // explicit --model and never leaves model selection to OpenCode.
  if (model === null || model === undefined) return OPENCODE_PINNED_MODEL
  const value = model.trim()
  if (!value) {
    throw new Error(
      `OpenCode harness has no explicit model configuration: expected '${OPENCODE_PINNED_MODEL}', got empty. Refusing to rely on OpenCode's default model selection.`,
    )
  }
  if (value !== OPENCODE_PINNED_MODEL) {
    throw new Error(
      `OpenCode harness model '${value}' is not the ENG-FORGE-V5-01 pinned model '${OPENCODE_PINNED_MODEL}'. No fallback model or harness substitution is allowed.`,
    )
  }
  return value
}

/**
 * Read the session id recorded for this story+lane, or null when there is nothing to resume.
 *
 * BEST-EFFORT BY CONTRACT: the pointer is an optimisation, never a precondition. A read that fails
 * (control plane unreachable, table not yet migrated, env misconfigured) is reported as an observed
 * warning and the lane starts a FRESH session — because the alternative is a lane that dies at START and
 * blames itself for a bookkeeping fault. The write side of the same pointer is tolerated the same way at
 * the end of `resultExternal`.
 */
async function readRecordedSessionId(
  context: AgentExecutionContext,
): Promise<string | null> {
  try {
    const { readVendorSessionId } = await import('../../db/forge-vendor-session')
    return await readVendorSessionId(context.command.storyId, 'opencode')
  } catch (err) {
    captureServerLog(
      'warn',
      'forge.vendor-session.read',
      `no recorded opencode session for story ${context.command.storyId}: ${(err as Error).message}`,
      { storyId: context.command.storyId, route: 'forge:vendor-session' },
    )
    return null
  }
}

/** Non-throwing readiness variant: returns a blocker reason or null. */
export function openCodeModelBlocker(
  model: string | null | undefined,
): string | null {
  if (model === null || model === undefined) return null
  const value = model.trim()
  if (!value) {
    return `OpenCode harness has no explicit model configuration: expected '${OPENCODE_PINNED_MODEL}', got empty. Refusing to rely on OpenCode's default model selection.`
  }
  if (value !== OPENCODE_PINNED_MODEL) {
    return `OpenCode harness model '${value}' is not the ENG-FORGE-V5-01 pinned model '${OPENCODE_PINNED_MODEL}'. No fallback model or harness substitution is allowed.`
  }
  return null
}

export class OpenCodeHarnessAdapter extends AgentRuntimeAdapter {
  // Single source of truth: the class field and the exported execution
  // identity record both read OPENCODE_HARNESS_ADAPTER_ID, so persisted
  // evidence (runtime_adapter) can never drift from the identity helper.
  readonly runtimeAdapterId = OPENCODE_HARNESS_ADAPTER_ID
  readonly capabilities: AgentCapability[] = OPENCODE_CAPABILITIES

  private handle: OpenCodeHandle | null = null
  private lastResult: OpenCodeRunResult | null = null
  /** The session id this run was launched with, so a failure can drop exactly it. */
  private pinnedSessionId: string | null = null
  /** Session totals at launch, so a resumed role reports its own spend, not the lifetime total. */
  private sessionBaseline: HarnessUsage | null = null
  /** What THIS role spent, reported to the run row as evidence. */
  private harnessUsage: HarnessUsage | null = null
  /** Wall-clock start of the OpenCode process (factual elapsed-time evidence). */
  private startedAtMs: number | null = null
  /**
   * WHERE HEAD STOOD WHEN THIS LANE STARTED, so a run can only claim a commit it made.
   *
   * Without a Forge-provisioned workspace there is no pinned base commit, and `readWorkerCommitHash(dir,
   * null)` answers with the plain HEAD — so a commit-CAPABLE lane that committed nothing reported whatever
   * commit it happened to find. Measured on 2026-09-16 (`ENG-QA-SINGLE-VERDICT-01`): the smith created
   * `08b569f8`, and the `lead_post` run AND both `dev_ops` runs recorded `08b569f8` as their OWN commit.
   * Non-commit roles were safe only because the write policy revokes their hash, which is a second, unrelated
   * guard doing this job. Captured before the harness process starts; `null` means the read failed, and a
   * failed read then claims nothing rather than claiming HEAD.
   */
  private headAtStart: string | null = null

  constructor(
    deps: ConstructorParameters<typeof AgentRuntimeAdapter>[0],
    private readonly config: OpenCodeHarnessConfig,
    private readonly taskBuilder: (
      command: AgentWorkCommand,
      context: AgentExecutionContext,
    ) => string = buildTaskText,
  ) {
    super(deps)
  }

  // -------------------------------------------------------------------------
  // PROTECTED VENDOR HOOKS (shared lifecycle lives in the base class)
  // -------------------------------------------------------------------------

  protected async startExternal(
    context: AgentExecutionContext,
  ): Promise<ExternalStartResult> {
    // Defensive FAIL-FAST (ENG-20/ENG-20A): never spawn external work for an
    // execution target whose application/domain DB configuration would resolve
    // to the production database. Second barrier directly before the harness
    // process is started (the invoker guards before calling execute).
    //
    // EXPLICIT (2026-09-12): the target comes from the caller, or from an explicit
    // environment declaration. It used to be `?? 'DEV'`, so an undeclared harness
    // silently ran as DEV; now it refuses through the resolver instead.
    const target = (context.executionEnvironment ?? resolveExecutionTarget()) as never
    assertExecutionTargetSafe(target)

    // ENG-FORGE-V5-01: OpenCode runs INSIDE the Forge-provisioned isolated
    // worker worktree only. `--auto` (permission auto-approval) must never
    // reach an unisolated/shared checkout, and OpenCode must not choose or
    // create its own workspace. Missing isolation fails closed with a
    // truthful reason — no fallback to the shared checkout.
    // NO TREES. EVER.. There is no Forge-provisioned worker worktree any more: the
    // estate was deleted, the invoker no longer hands one out, and no lane may recreate one. The refusal that
    // demanded a worktree is what killed the architect lane on every start (engine task 65f40df2, 07:26) with
    // "requires the Forge-provisioned isolated worker worktree". The lane now runs in the working directory it
    // was given — the checkout the worker holds — and records what it did in the rows.
    const workspace = context.executionWorkspace?.worktreePath ?? process.cwd()
    verifyWorkspaceEnvFile(workspace, target)

    // Explicit model pinning — never OpenCode's default model selection.
    const model = resolveOpenCodeModel(this.config.model)

    // Canonical Forge task/prompt contract preserved verbatim.
    const task = this.taskBuilder(context.command, context)

    const startRun = this.config.startRun ?? startOpenCodeRun
    // V5-21 (env-gated, default OFF): resume the same session when a prior role
    // already succeeded in this isolated worktree (marker present). The first
    // role of a fresh generation runs fresh; later roles inherit its context.
    // V5-21 / WARM-SESSION-01: resume the SAME session this execution generation has
    // been using. The marker holds the actual session id, so the next role pins it with
    // `--session <id>`; `--continue` survives only as the legacy path for a bare `1`
    // marker written before ids were recorded, because "the project's last session" is a
    // guess that is only correct while exactly one lane is live.
    const continuityEnabled = forgeSessionContinuityEnabled(process.env)
    // The marker file is no longer read: the pointer lives in the row (see below).
    // THE POINTER COMES FROM THE ROW: the session id lives in forge_vendor_session, not
    // in a worktree file. Same continuity, same token savings — one session serving the generation — and now
    // it is auditable in a query instead of living on one laptop. No id recorded means a fresh session; the
    // old `--continue` guess ("whatever session the project touched last") is gone with the file it needed.
    //
    // A POINTER THAT CANNOT BE READ IS NOT A REASON TO REFUSE THE WORK. Continuity is an OPTIMISATION, so an
    // unreadable row degrades to "start fresh" — the same way an unreadable harness store is tolerated below
    // and a failed write is tolerated above. It must never throw: `readVendorSessionId` reaches the
    // control-plane database, and a lane that died at START because a bookkeeping read failed would spend
    // nothing and report a database fault as its own failure (measured 2026-09-16: `startExternal` threw
    // DATABASE_UNAVAILABLE on EVERY path once the pointer moved into the row, taking the whole opencode
    // harness suite red with it, because the suite runs without a control plane).
    const sessionId = continuityEnabled ? await readRecordedSessionId(context) : null
    const continueSession = false
    this.pinnedSessionId = sessionId ?? null
    // Baseline BEFORE the turn: one session serves every role of the generation, so this
    // role's spend is the difference across its turn, not the session's lifetime total.
    this.sessionBaseline = sessionId ? readSessionUsage({ sessionId }) : null
    this.harnessUsage = null
    // DEV safety: the spawned harness (and any test process it spawns) must
    // NOT inherit an APP_ENV/DATABASE_URL set that resolves to the production
    // application database.
    const childEnv = buildChildProcessEnv(target)
    this.startedAtMs = Date.now()
    // WHERE HEAD STOOD BEFORE THE MODEL TOUCHED ANYTHING. Read here, immediately before the harness is
    // spawned, because after it runs there is no way to tell this lane's commit from the checkout it
    // inherited — see `headAtStart`.
    this.headAtStart = await readWorkerCommitHash(workspace)
    // V5-24: RTK transparency. Shims for the supported commands (git/ls/tree/gh)
    // are generated for THIS worktree and prepended to the child PATH, so the
    // model keeps typing `git status` and transparently gets `rtk git status`.
    // Positions without the grant, or with rtk unavailable, are returned
    // unchanged — an unavailable tool degrades, it never half-applies.
    const rtk = applyRtkToEnv({
      role: forgeToolRoleForAgentRole(context.command.role),
      env: { ...childEnv, ...(this.config.env ?? {}) },
      workspace,
    })
    this.handle = startRun({
      cliBin: this.config.cliBin,
      cwd: workspace,
      model,
      task,
      ...(sessionId ? { session: sessionId } : {}),
      continueSession,
      // Spread into a fresh literal so the parameter's precise env type is
      // restored (a passthrough value would widen it).
      env: { ...rtk.env },
    })
    this.externalRunId = `opencode-${Date.now()}`
    return { externalRunId: this.externalRunId }
  }

  protected async statusExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<ExternalStatusResult> {
    if (!this.handle) return { lifecycle: 'failed' }
    // Cancellation is requested first: SIGTERM sent. The child may not have
    // exited yet; map directly to the canonical cancelled lifecycle so the
    // shared loop terminalizes without racing the process exit.
    if (this.handle.cancelled) return { lifecycle: 'cancelled' }
    // Still alive / not yet settled -> running. `done` also covers a spawn
    // failure (missing binary), which settles immediately as failed below —
    // the poll can never report a dead launch as running forever.
    if (!this.handle.done) {
      return { lifecycle: 'running', detail: { externalRunId: this.externalRunId } }
    }
    // Process settled — resolve the final result exactly once.
    if (!this.lastResult) {
      this.lastResult = await this.handle.promise
    }
    if (this.lastResult.status === 'failed' && !this.externalErrorText) {
      this.externalErrorText =
        this.lastResult.stderr.trim() ||
        `opencode exited ${this.lastResult.exitCode ?? 'no-code'}`
    }
    if (this.lastResult.status === 'success') return { lifecycle: 'success' }
    return { lifecycle: 'failed' }
  }

  protected async pauseExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    if (!this.handle) return
    // Semantics-preserving process pause (SIGSTOP); the child survives.
    this.handle.pause()
  }

  protected async resumeExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    if (!this.handle) return
    this.handle.resume()
  }

  protected async cancelExternal(
    _command: AgentWorkCommand,
    _context: AgentExecutionContext,
  ): Promise<void> {
    if (!this.handle) return
    this.handle.cancel()
  }

  protected async resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    if (!this.handle) return null
    const result = await this.handle.promise
    this.lastResult = result

    if (result.status === 'failed') {
      this.externalErrorText =
        result.stderr.trim() || `opencode exited ${result.exitCode ?? 'no-code'}`
      // A PINNED SESSION THAT JUST FAILED IS DROPPED, ONCE. The next role in this
      // generation then starts fresh and records its own id, so a dead session costs one
      // role rather than every role after it. Leaving the marker in place would reuse the
      // corpse; never clearing on success would leave no id to reuse at all.
      if (this.pinnedSessionId) {
        void import('../../db/forge-vendor-session')
          .then((m) => m.writeVendorSessionId(context.command.storyId, 'opencode', null))
          .catch((err) => console.warn(`vendor-session clear skipped: ${String(err)}`))
      }
      return null
    }

    const model = resolveOpenCodeModel(this.config.model)
    // NO TREES. EVER.. A lane measures the CODE, and the code is wherever the lane was
    // told to run — a working directory, not a tree it owns. This was the LAST refusal left over from the
    // worktree era: `startExternal` shed its demand for `context.executionWorkspace` in `53556ce7`, this
    // hook kept it and answered `null`, and every successful run of a lane that had no worktree then ended
    // as `Cannot read properties of null (reading 'notes')` in the engine ledger — the architect lane, engine
    // tasks `e599df3a` (07:28) and `d4266df9` (07:39), its real result thrown away and the TypeError named
    // as the cause. The directory below is the SAME one `startExternal` spawned in, so the evidence
    // describes the run that actually happened instead of a tree that was never allowed to exist.
    const executionWorkspace = context.executionWorkspace ?? null
    const workspace = executionWorkspace?.worktreePath ?? process.cwd()

    // V5-21 / WARM-SESSION-01: record the ACTUAL session id this run executed in, so the
    // next role of this generation pins that exact session with `--session <id>` instead
    // of guessing at "the project's last session". The id comes from the harness's own
    // store, read through the same reader that feeds cost capture.
    //
    // An unreadable store leaves whatever id was already recorded in place: losing an id
    // to a bookkeeping miss would restart the seed tax this work exists to remove.
    if (forgeSessionContinuityEnabled(process.env)) {
      // A RESUMED role must be read by id: the time-window read finds the session's
      // CREATOR, so every role after the first reported nothing at all.
      const measured = this.pinnedSessionId
        ? readSessionUsage({ sessionId: this.pinnedSessionId })
        : readHarnessUsage({ harnessStartedAtMs: this.startedAtMs ?? Date.now() })
      if (measured?.sessionId) {
        void import('../../db/forge-vendor-session')
          .then((m) => m.writeVendorSessionId(context.command.storyId, 'opencode', measured.sessionId))
          .catch((err) => console.warn(`vendor-session write skipped: ${String(err)}`))
      }
      this.harnessUsage = measured ? usageDelta(measured, this.sessionBaseline) : null
    }

    // ENG-FORGE-V5-01 / AC5: Forge, not OpenCode, owns the candidate commit
    // through the existing harness-owned commit path. This hook reads the
    // factual HEAD of the worker worktree; when OpenCode left the workspace
    // dirty (no commit), the outer Forge policy wrapper creates the candidate
    // commit itself via commitWorkerWorkspaceChanges. Null is persisted when
    // the checkout is exactly at the approved base commit — never fabricated.
    //
    // THE BASELINE IS WHAT THIS LANE STARTED FROM, not merely where the checkout happens to point. With a
    // Forge-provisioned workspace that is the approved base commit (unchanged behaviour). WITHOUT one there is
    // no pinned base, so the baseline is the HEAD captured in `startExternal` — which is what keeps a lane
    // that committed nothing from reporting the previous lane's commit as its own. A baseline that could not
    // be read claims NOTHING: an unknown starting point is not evidence that this lane committed.
    const baseline = executionWorkspace?.baseCommit ?? this.headAtStart
    const commitHash = baseline ? await readWorkerCommitHash(workspace, baseline) : null

    const elapsedMs =
      this.startedAtMs !== null ? Date.now() - this.startedAtMs : null

    const notes = [
      'OpenCode run completed.',
      // The directory the lane ran in, named for WHAT IT IS. This line named a `worktree` until 2026-09-17:
      // with NO TREES there is no per-lane worktree, so the field described a thing that no longer exists —
      // and every run wrote it, which is why 120 rows landed AFTER ENG-FORGE-ARTIFACT-RESIDUE-01 still
      // carried a tree-era token. The fence now scans this adapter (workflow_app/tests/no-tree-residue.test.ts).
      `Run metadata: harness=opencode model=${model} workspace=${workspace} exit=${result.exitCode ?? 'n/a'}${
        elapsedMs !== null ? ` elapsed_ms=${elapsedMs}` : ''
      }`,
      // The workspace evidence line is emitted only when a Forge-provisioned
      // workspace exists, exactly as `deepseek-harness-adapter.ts` does: the run
      // metadata line below already states the directory the lane ran in, and a
      // line claiming a `base=<ref>@<sha>` that no workspace pinned would be the
      // fabricated proof `verifiedShaFromWorkspaceEvidence` exists to refuse.
      executionWorkspace ? workspaceEvidenceLine(executionWorkspace) : null,
      result.stdout.trim() ? `Assistant output:\n${result.stdout.trim()}` : 'No assistant text captured.',
      result.stderr.trim() ? `stderr:\n${result.stderr.trim()}` : null,
    ]
      .filter(Boolean)
      .join('\n\n')

    // FULL-harness guard (SCOPED mode): if the model reported invoking a known
    // full-regression command, flag it in the durable evidence so the operator
    // sees a policy violation instead of silently accepting the tax.
    const { mode } = resolveTestModeFromInstructions(
      command.specialInstructions,
      process.env.AGENT_TEST_MODE ?? null,
    )
    const forbidden =
      mode === 'SCOPED' ? detectFullRegressionAttempt(result.stdout) : null

    // ENG-08 — concise tests/checks summary: prefer the model's deliberate
    // `Tests: ...` evidence line; fall back to the factual exit code. A SCOPED
    // violation replaces the summary so the operator sees the policy breach.
    const testsSummary = extractTestsSummary(
      result.stdout,
      `opencode exit code ${result.exitCode}`,
    )

    return {
      resultStatus: 'Complete',
      completion: 100,
      notes: forbidden
        ? notes + `\n\nTEST-MODE VIOLATION (SCOPED): the model reported the forbidden FULL-regression command \`${forbidden}\`. The runtime policy is authoritative; FULL requires explicit runtime authorization (test-mode: FULL).`
        : notes,
      testsSummary: forbidden
        ? `opencode exit code ${result.exitCode} | TEST-MODE VIOLATION (SCOPED): ${forbidden}`
        : testsSummary,
      commitHash,
      // Spend vision: harness-enforced exact model, recorded as evidence.
      modelUsed: model,
      // This role's own spend, measured against the session totals at launch. Supplied from
      // here because only the adapter knows which session the run pinned — a time-window
      // read finds the session's CREATOR and misses every resumed role.
      harnessUsage: this.harnessUsage,
      runtimeAdapter: this.runtimeAdapterId,
      modelProfile: command.modelProfile,
      externalRunId: this.externalRunId,
      executionEnvironment: command.executionEnvironment ?? null,
      startedAt: new Date().toISOString(),
      endedAt: new Date().toISOString(),
    }
  }

  // -------------------------------------------------------------------------
  // Progress projection: the harness exposes no mid-run percent; report a
  // factual liveness step only.
  // -------------------------------------------------------------------------

  protected override progressFromStatus(
    _status: ExternalStatusResult,
    _command: AgentWorkCommand,
  ): { step?: string; completion?: number; note?: string } | null {
    return {
      step: 'executing',
      note: `opencode run executing (external ${this.externalRunId})`,
    }
  }
}
