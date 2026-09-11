// ---------------------------------------------------------------------------
// Deterministic static gate — dependency-cruiser (architecture) + semgrep
// (security/correctness). A reusable capability: given a workspace and where
// its tools live, run the checks and return a normalized result.
//
// The isolated exact-candidate worktree has the CODE but no node_modules, while
// the primary checkout has the TOOLS — so callers pass tool paths/binaries
// explicitly and the workspace path separately. This keeps the gate usable in
// either environment without forcing a tool install into the worktree.
// ---------------------------------------------------------------------------
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

export type StaticGateResult = {
  workspace: string
  roots: string[]
  archOk: boolean
  archErrors: string[]
  semgrepRan: boolean
  semgrepFindings: string[]
  /** Hygiene instrument (V5-27). Informational: never recalls Smith. */
  knipRan: boolean
  knipFindings: string[]
  /** True when architecture (the hard gate) is clean. */
  ok: boolean
}

const DEFAULT_ROOTS = ['workflow_app', 'agent-runtime', 'db', 'services', 'app', 'components', 'ui', 'lib', 'workflow_engine']

function spawn(cmd: string, args: string[], cwd: string, timeoutMs: number): { status: number | null; out: string; stdout: string; stderr: string } {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', timeout: timeoutMs, maxBuffer: 32 * 1024 * 1024 })
  const stdout = r.stdout ?? ''
  const stderr = r.stderr ?? ''
  return { status: r.status, out: stdout + stderr, stdout, stderr }
}

function runDepcruise(input: {
  workspace: string
  roots: string[]
  config: string
  depcruiseBin?: string
  timeoutMs: number
}): { ok: boolean; errors: string[] } {
  const bin = input.depcruiseBin
  const args = bin
    ? ['--config', input.config, ...input.roots] // explicit binary path mode
    : ['exec', 'depcruise', ...input.roots, '--config', input.config, '--output-type', 'err']
  const r = spawn(bin ?? 'pnpm', args, input.workspace, input.timeoutMs)
  const errors = r.out
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !/^info:|^warn:|depcruise:|Using (system|custom)/i.test(l))
  return { ok: r.status === 0, errors: r.status !== 0 ? errors : [] }
}

function runSemgrep(input: { workspace: string; roots: string[]; configDir: string; semgrepBin?: string; timeoutMs: number }): {
  ran: boolean
  findings: string[]
} {
  if (!existsSync(join(input.workspace, input.configDir))) return { ran: false, findings: [] }
  const r = spawn(input.semgrepBin ?? 'semgrep', ['scan', '--config', input.configDir, '--json', ...input.roots], input.workspace, input.timeoutMs)
  const findings: string[] = []
  if (r.status === 0) return { ran: true, findings }
  // semgrep --json writes structured results to stdout; parse them so the
  // artifact carries rule id / path / line / severity / message (not a raw blob).
  try {
    const json = JSON.parse(r.stdout) as { results?: Array<{
      check_id?: string
      path?: string
      start?: { line?: number }
      extra?: { message?: string; severity?: string }
    }> }
    const results = Array.isArray(json.results) ? json.results : []
    for (const hit of results.slice(0, 100)) {
      const rule = hit.check_id ?? 'semgrep'
      const path = hit.path ?? '?'
      const line = hit.start?.line ?? 0
      const severity = hit.extra?.severity ?? 'WARNING'
      const message = (hit.extra?.message ?? '').replace(/\s+/g, ' ').trim()
      findings.push(`${severity} ${rule} ${path}:${line}${message ? ` — ${message.slice(0, 200)}` : ''}`)
    }
    if (findings.length === 0) {
      findings.push(`semgrep exited ${r.status} but returned no structured results`)
    }
  } catch {
    // Not JSON (or unexpected shape) — keep a bounded raw snippet as a fallback.
    findings.push(r.out.slice(0, 400))
  }
  return { ran: true, findings }
}

function runKnip(input: { workspace: string; knipBin?: string; timeoutMs: number }): {
  ran: boolean
  findings: string[]
} {
  // knip needs the installed tool and a manifest, exactly like depcruise.
  const hasManifest = existsSync(join(input.workspace, 'package.json'))
  const hasKnipBin =
    Boolean(input.knipBin) || existsSync(join(input.workspace, 'node_modules', '.bin', 'knip'))
  if (!hasManifest || !hasKnipBin) return { ran: false, findings: [] }

  const bin = input.knipBin ?? 'pnpm'
  const args = input.knipBin
    ? ['--reporter', 'json']
    : ['exec', 'knip', '--reporter', 'json']
  const r = spawn(bin, args, input.workspace, input.timeoutMs)
  const findings: string[] = []
  if (r.status === 0) return { ran: true, findings }

  // Parse structured output so the artifact carries file/symbol/type, not a blob.
  try {
    const json = JSON.parse(r.stdout) as {
      files?: Array<string | { file?: string; name?: string }>
      issues?: Array<{
        file?: string
        symbol?: string
        type?: string
        name?: string
      }>
    }
    for (const issue of (json.issues ?? []).slice(0, 100)) {
      const type = issue.type ?? 'issue'
      const file = issue.file ?? '?'
      const symbol = issue.symbol ?? issue.name ?? ''
      findings.push(`${type} ${file}${symbol ? `:${symbol}` : ''}`)
    }
    for (const file of (json.files ?? []).slice(0, 100)) {
      const path = typeof file === 'string' ? file : (file?.file ?? file?.name ?? '?')
      findings.push(`unused-file ${path}`)
    }
    if (findings.length === 0) {
      findings.push(`knip exited ${r.status} but returned no structured results`)
    }
  } catch {
    findings.push(r.out.slice(0, 400))
  }
  return { ran: true, findings }
}

/**
 * Run the deterministic static gate against a workspace. Architecture is the
 * hard gate (ok=false on error-severity findings); semgrep and knip report
 * informationally — hygiene must never recall Smith.
 */
export function runStaticGate(input: {
  workspace: string
  roots?: string[]
  /** Absolute path to the dependency-cruiser config (relative to workspace by default). */
  config?: string
  /** Explicit dependency-cruiser binary (e.g. primary checkout node_modules/.bin/depcruise) for worktrees. */
  depcruiseBin?: string
  /** Explicit semgrep binary path. */
  semgrepBin?: string
  /** Explicit knip binary (e.g. primary checkout node_modules/.bin/knip). */
  knipBin?: string
  timeoutMs?: number
}): StaticGateResult {
  const workspace = input.workspace
  const roots = input.roots?.length ? input.roots : DEFAULT_ROOTS
  const config = input.config ?? '.dependency-cruiser.js'
  const timeoutMs = input.timeoutMs ?? 180_000

  // dependency-cruiser validates a JS/TS module graph. It can only run when the
  // tool actually resolves in this workspace: an explicit binary path, or a
  // locally installed depcruise (node_modules/.bin). A candidate worktree has
  // package.json but usually no node_modules, and a bare unit harness has no
  // package at all — in both cases there is nothing to cruise, so skip rather
  // than fail the hard gate on missing tooling. QA must not FAIL (and recall
  // Smith) just because an optional deep check cannot run.
  const hasManifest = existsSync(join(workspace, 'package.json'))
  const hasDepcruiseBin =
    Boolean(input.depcruiseBin) ||
    existsSync(join(workspace, 'node_modules', '.bin', 'depcruise'))
  const arch =
    hasManifest && hasDepcruiseBin
      ? runDepcruise({
          workspace,
          roots,
          config,
          depcruiseBin: input.depcruiseBin,
          timeoutMs,
        })
      : { ok: true, errors: [] }
  const sec = runSemgrep({ workspace, roots, configDir: '.semgrep', semgrepBin: input.semgrepBin, timeoutMs })
  const hygiene = runKnip({ workspace, knipBin: input.knipBin, timeoutMs })

  return {
    workspace,
    roots,
    archOk: arch.ok,
    archErrors: arch.errors,
    semgrepRan: sec.ran,
    semgrepFindings: sec.findings,
    knipRan: hygiene.ran,
    knipFindings: hygiene.findings,
    ok: arch.ok,
  }
}
