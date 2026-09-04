// ---------------------------------------------------------------------------
// dsh-client — minimal process wrapper around the DeepSeek Harness headless
// CLI (ENG-19). DeepSeek-specific mechanics are isolated HERE, beneath the
// vendor-neutral AgentRuntimeAdapter boundary. Nothing in this module is
// imported by the generic command model, invoker, Story Board, or
// workflow_engine.
//
// Factual harness behavior (inspected from the actual installed source):
//   - Invocation: `dsh --profile headless "<task>"` creates a fresh session,
//     drives the agent to completion, prints the final assistant text to
//     stdout, and exits (code 0 = completed, 1 = error).
//   - Session ID: generated internally as `session-<uuid>` and persisted
//     under `$DSH_HOME/sessions/<project>/session-<uuid>/session.jsonl.zstd`.
//     It is NOT printed by the CLI; the parent discovers it as the newest
//     `session-*` directory for the workspace after (or during) the run.
//   - Status: a running invocation is an alive child process. Terminal =
//     process exit. There is no polling API mid-run for headless mode.
//   - Pause/Resume: no native headless pause/resume. A semantics-preserving
//     process wrapper is possible via SIGSTOP/SIGCONT (the child process and
//     its session survive; the run simply suspends). Exposed here.
//   - Cancel: SIGTERM the child; the partial session JSONL (if any) remains
//     on disk. The parent marks canonical Cancelled state.
//   - Heartbeat/liveness: OUR lease renewal is authoritative; this client only
//     reports process liveness.
//   - Result/evidence: stdout = final assistant text; stderr = harness errors;
//     exit code = 0/1. Session transcript lives in the session directory.
// ---------------------------------------------------------------------------

import { spawn, type ChildProcess } from 'node:child_process'
import { readdirSync, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

export type DshRunStatus = 'running' | 'success' | 'failed'

export type DshRunResult = {
  status: DshRunStatus
  exitCode: number | null
  stdout: string
  stderr: string
  sessionId: string | null
  sessionDir: string | null
}

export type DshStartOptions = {
  /** Absolute path to the DeepSeek Harness CLI bin (lib/bin.js). */
  cliBin: string
  /** Workspace cwd the harness agent operates in. */
  cwd: string
  /** Task text. */
  task: string
  /** Absolute path to a `--patch` overlay (e.g. model pin), or null. */
  modelPatchFile?: string | null
  /** Optional env additions (e.g. DEEPSEEK_API_KEY passthrough). */
  env?: Record<string, string | undefined>
}

export type DshClient = {
  start(opts: DshStartOptions): DshHandle
  /** Discover the newest session dir for a workspace (correlation recovery). */
  discoverLatestSession(workspacePath: string): string | null
}

export type DshHandle = {
  /** Child process (liveness + cancel). */
  proc: ChildProcess
  /** Resolve when the run completes. */
  promise: Promise<DshRunResult>
  /** True once cancel() has been requested (SIGTERM sent). */
  cancelled: boolean
  /** SIGSTOP (pause) — semantics-preserving process suspension. */
  pause(): void
  /** SIGCONT (resume). */
  resume(): void
  /** SIGTERM (cancel). */
  cancel(): void
}

/** Resolve `$DSH_HOME` (default `~/.dsh`). */
export function dshHome(): string {
  return process.env.DSH_HOME ?? join(homedir(), '.dsh')
}

/** Resolve the project-scoped sessions root for a workspace path.
 * Replicates the harness's exact `projectKey` escaping (verified in
 * deepseek-harness/packages/session/session-persistence-jsonl/src/format.ts):
 * separators `/` `\` `:` become a single `-` (runs collapsed), leading `-`
 * is stripped, other unsafe code units become `~XXXX`, and the result is
 * wrapped in `--...--`.
 */
export function sessionRootForWorkspace(workspacePath: string): string {
  let readable = ''
  let separatorRun = false
  for (let i = 0; i < workspacePath.length; i++) {
    const ch = workspacePath[i]
    if (ch === '/' || ch === '\\' || ch === ':') {
      if (!separatorRun) readable += '-'
      separatorRun = true
    } else if (ch !== '~' && /^[A-Za-z0-9._-]$/.test(ch)) {
      readable += ch
      separatorRun = false
    } else {
      readable += '~' + workspacePath.charCodeAt(i).toString(16).toUpperCase().padStart(4, '0')
      separatorRun = false
    }
  }
  const slug = readable.replace(/^-+/, '') || 'root'
  return join(dshHome(), 'sessions', `--${slug.slice(0, 251)}--`)
}


/** Newest `session-*` directory name for a workspace (null if none). */
export function discoverLatestSession(workspacePath: string): string | null {
  const root = sessionRootForWorkspace(workspacePath)
  let entries: string[]
  try {
    entries = readdirSync(root)
  } catch {
    return null
  }
  const sessions = entries
    .filter((e) => e.startsWith('session-'))
    .map((e) => {
      let mtime = 0
      try {
        mtime = statSync(join(root, e)).mtimeMs
      } catch {
        mtime = 0
      }
      return { name: e, mtime }
    })
    .sort((a, b) => b.mtime - a.mtime)
  return sessions[0]?.name ?? null
}

/**
 * Start one headless DeepSeek Harness run as a child process. Returns a handle
 * immediately (for pause/resume/cancel + liveness) and a promise that resolves
 * with the normalized result.
 */
export function startDshRun(opts: DshStartOptions): DshHandle {
  const workspace = resolve(opts.cwd)
  const sessionBefore = discoverLatestSession(workspace)
  // The child env is the AUTHORITATIVE env passed by the caller (the adapter
  // passes the execution-target sanitized env — APP_ENV/DATABASE_URL forced to
  // the DEV target, PROD url removed). Never blindly re-merge process.env here:
  // that would resurrect DATABASE_URL_PROD in a DEV child.
  const env: NodeJS.ProcessEnv = opts.env
    ? ({ ...opts.env } as NodeJS.ProcessEnv)
    : { ...process.env }
  const args = ['--profile', 'headless']
  if (opts.modelPatchFile?.trim()) {
    args.push('--patch', opts.modelPatchFile.trim())
  }
  args.push(opts.task)
  const proc = spawn(opts.cliBin, args, {
    cwd: opts.cwd,
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
  })

  let stdout = ''
  let stderr = ''
  let cancelled = false
  proc.stdout?.on('data', (d: Buffer) => {
    stdout += d.toString()
  })
  proc.stderr?.on('data', (d: Buffer) => {
    stderr += d.toString()
  })

  const promise = new Promise<DshRunResult>((resolvePromise) => {
    proc.on('error', (err) => {
      resolvePromise({
        status: 'failed',
        exitCode: null,
        stdout,
        stderr: stderr || String(err.message ?? err),
        sessionId: null,
        sessionDir: null,
      })
    })
    proc.on('close', (code) => {
      const latest = discoverLatestSession(workspace)
      let sessionId: string | null = null
      let sessionDir: string | null = null
      if (latest) {
        sessionId = latest
        sessionDir = join(sessionRootForWorkspace(workspace), latest)
      }
      resolvePromise({
        status: code === 0 ? 'success' : 'failed',
        exitCode: code,
        stdout,
        stderr,
        sessionId,
        sessionDir,
      })
    })
  })

  return {
    proc,
    promise,
    cancelled,
    pause: () => proc.kill('SIGSTOP'),
    resume: () => proc.kill('SIGCONT'),
    cancel: () => {
      cancelled = true
      proc.kill('SIGTERM')
    },
  }
}

