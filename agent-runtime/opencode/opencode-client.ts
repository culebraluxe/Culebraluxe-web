// ---------------------------------------------------------------------------
// opencode-client — minimal process wrapper around the installed OpenCode CLI
// (ENG-FORGE-V5-01). OpenCode-specific mechanics are isolated HERE, beneath
// the vendor-neutral AgentRuntimeAdapter boundary. Nothing in this module is
// imported by the generic command model, invoker, Story Board, or
// workflow_engine.
//
// Factual OpenCode CLI behavior (verified against `opencode run --help`,
// opencode v1.x):
//   - Invocation: `opencode run --model <provider/model> --auto "<task>"`
//     runs ONE non-interactive agent session in the current working directory
//     and exits 0 when the run completes (non-zero on error).
//   - `--auto` auto-approves permission prompts. It is SAFE ONLY inside the
//     Forge-provisioned isolated worker worktree; the adapter refuses to run
//     outside an isolated workspace, so `--auto` never reaches an unisolated
//     checkout.
//   - The model is ALWAYS passed explicitly via `--model`. OpenCode's own
//     default/automatic model selection is never exercised.
//   - The task is the canonical Forge task/prompt contract passed verbatim as
//     the run message.
//   - There is no mid-run status API: running == an alive child process.
//     Pause/Resume = semantics-preserving SIGSTOP/SIGCONT; cancel = SIGTERM.
//     Result/evidence: stdout = assistant text; stderr = logs/errors; exit
//     code = terminal status. Forge never discovers/manages OpenCode sessions.
// ---------------------------------------------------------------------------

import { spawn, type ChildProcess } from 'node:child_process'

export type OpenCodeRunStatus = 'running' | 'success' | 'failed'

export type OpenCodeRunResult = {
  status: OpenCodeRunStatus
  exitCode: number | null
  stdout: string
  stderr: string
}

export type OpenCodeStartOptions = {
  /** Executable to run: `opencode` (PATH) or an absolute entrypoint. */
  cliBin: string
  /** Exact worker workspace (cwd) the OpenCode agent operates in. */
  cwd: string
  /** Explicit model id (`provider/model`). Never left to OpenCode defaults. */
  model: string
  /** Canonical Forge task/prompt text passed as the run message. */
  task: string
  /** Optional env additions (auth/API keys flow through the child env). */
  env?: Record<string, string | undefined>
  /** Auto-approve OpenCode permission prompts. Forge only enables this inside
   *  its isolated worker worktree; default true per the ENG-FORGE-V5-01
   *  contract (the adapter guarantees isolation before starting). */
  autoApprove?: boolean
}

export type OpenCodeHandle = {
  /** Child process (liveness + signals). */
  proc: ChildProcess
  /** True once the run has settled (close or spawn error). */
  done: boolean
  /** Resolve when the run completes with the normalized result. */
  promise: Promise<OpenCodeRunResult>
  /** True once cancel() has been requested (SIGTERM sent). */
  cancelled: boolean
  /** SIGSTOP (pause) — semantics-preserving process suspension. */
  pause(): void
  /** SIGCONT (resume). */
  resume(): void
  /** SIGTERM (cancel). */
  cancel(): void
}

/**
 * Build the `opencode run` argv. Pure and deterministic so tests can prove
 * command construction, explicit model pinning, and the non-interactive
 * `--auto` flag without spawning a process.
 */
export function buildOpenCodeRunArgs(input: {
  model: string
  task: string
  autoApprove?: boolean
}): string[] {
  const autoApprove = input.autoApprove ?? true
  return [
    'run',
    '--model',
    input.model,
    ...(autoApprove ? ['--auto'] : []),
    input.task,
  ]
}

/**
 * Start one non-interactive `opencode run` as a child process. Returns a
 * handle immediately (for pause/resume/cancel + liveness) and a promise that
 * resolves with the normalized result when the process settles.
 *
 * The child env is the AUTHORITATIVE env passed by the caller (the adapter
 * passes the execution-target sanitized env — APP_ENV/DATABASE_URL forced to
 * the DEV target, the PROD url removed). Never blindly re-merge process.env
 * here: that would resurrect DATABASE_URL_PROD in a DEV child.
 */
export function startOpenCodeRun(opts: OpenCodeStartOptions): OpenCodeHandle {
  const env: NodeJS.ProcessEnv = opts.env
    ? ({ ...opts.env } as NodeJS.ProcessEnv)
    : { ...process.env }
  const proc = spawn(
    opts.cliBin,
    buildOpenCodeRunArgs({
      model: opts.model,
      task: opts.task,
      autoApprove: opts.autoApprove ?? true,
    }),
    {
      cwd: opts.cwd,
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  )

  let stdout = ''
  let stderr = ''
  proc.stdout?.on('data', (chunk: Buffer) => {
    stdout += chunk.toString()
  })
  proc.stderr?.on('data', (chunk: Buffer) => {
    stderr += chunk.toString()
  })

  let handle: OpenCodeHandle = null as never
  const promise = new Promise<OpenCodeRunResult>((resolvePromise) => {
    // Spawn/launch failure (missing binary, EACCES, ...): settle immediately
    // as a FAILED result so the adapter's status poll can never report the
    // run as alive forever. Truthful failure evidence, no silent fallback.
    proc.on('error', (err) => {
      handle.done = true
      resolvePromise({
        status: 'failed',
        exitCode: null,
        stdout,
        stderr: stderr || String(err.message ?? err),
      })
    })
    proc.on('close', (code) => {
      handle.done = true
      resolvePromise({
        status: code === 0 ? 'success' : 'failed',
        exitCode: code,
        stdout,
        stderr,
      })
    })
  })

  handle = {
    proc,
    done: false,
    promise,
    cancelled: false,
    pause: () => proc.kill('SIGSTOP'),
    resume: () => proc.kill('SIGCONT'),
    cancel: () => {
      // Mutate the HANDLE's flag (not just the closure variable) so a status
      // poll that runs after cancel() reports the canonical cancelled state.
      handle.cancelled = true
      proc.kill('SIGTERM')
    },
  }
  return handle
}
