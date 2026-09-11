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
  /** Did dependency-cruiser actually execute? FALSE means the gate was skipped. */
  archRan: boolean
  archOk: boolean
  archErrors: string[]
  semgrepRan: boolean
  semgrepFindings: string[]
  /** Hygiene instrument (V5-27). Informational: never recalls Smith. */
  knipRan: boolean
  knipFindings: string[]
  /** Per-category counts (unused-file, unused-export, unused-dependency, ...). */
  knipCounts: Record<string, number>
  knipGroupCount: number
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

/** knip reports one group per file, with typed arrays inside. */
type KnipGroup = Record<string, unknown>

/** Category label -> the key knip uses. */
const KNIP_CATEGORIES: ReadonlyArray<[key: string, label: string]> = [
  ['files', 'unused-file'],
  ['exports', 'unused-export'],
  ['types', 'unused-type'],
  ['classMembers', 'unused-class-member'],
  ['enumMembers', 'unused-enum-member'],
  ['namespaceMembers', 'unused-namespace-member'],
  ['dependencies', 'unused-dependency'],
  ['devDependencies', 'unused-dev-dependency'],
  ['optionalPeerDependencies', 'unused-optional-peer'],
  ['unlisted', 'unlisted-dependency'],
  ['binaries', 'unlisted-binary'],
  ['unresolved', 'unresolved-import'],
  ['duplicates', 'duplicate-export'],
]

function knipItemName(item: unknown): string {
  if (typeof item === 'string') return item
  if (item && typeof item === 'object') {
    const row = item as Record<string, unknown>
    const name = row.name ?? row.symbol ?? row.file ?? row.line
    if (typeof name === 'string' || typeof name === 'number') {
      const parent = typeof row.parentName === 'string' ? `${row.parentName}.` : ''
      return `${parent}${String(name)}`
    }
  }
  return '?'
}

/**
 * Flatten knip's per-file groups into actionable findings, plus a per-category
 * count so the scale is visible without reading every line.
 */
export function parseKnipJson(stdout: string, limit = 200): {
  findings: string[]
  counts: Record<string, number>
  groupCount: number
} {
  const counts: Record<string, number> = {}
  const findings: string[] = []
  let parsed: { issues?: KnipGroup[] }
  try {
    parsed = JSON.parse(stdout) as { issues?: KnipGroup[] }
  } catch {
    return { findings, counts, groupCount: 0 }
  }
  const groups = Array.isArray(parsed.issues) ? parsed.issues : []
  for (const group of groups) {
    const file = typeof group.file === 'string' ? group.file : '?'
    for (const [key, label] of KNIP_CATEGORIES) {
      const items = group[key]
      if (!Array.isArray(items) || items.length === 0) continue
      counts[label] = (counts[label] ?? 0) + items.length
      for (const item of items) {
        if (findings.length >= limit) continue
        const name = knipItemName(item)
        findings.push(label === 'unused-file' ? `${label} ${file}` : `${label} ${file}:${name}`)
      }
    }
  }
  return { findings, counts, groupCount: groups.length }
}


function runKnip(input: { workspace: string; knipBin?: string; timeoutMs: number }): {
  ran: boolean
  findings: string[]
  counts: Record<string, number>
  groupCount: number
} {
  // knip needs the installed tool and a manifest, exactly like depcruise.
  const hasManifest = existsSync(join(input.workspace, 'package.json'))
  // Same rule as depcruise: an explicit path counts only if it exists.
  const hasKnipBin =
    (input.knipBin ? existsSync(input.knipBin) : false) ||
    existsSync(join(input.workspace, 'node_modules', '.bin', 'knip'))
  if (!hasManifest || !hasKnipBin) return { ran: false, findings: [], counts: {}, groupCount: 0 }

  const bin = input.knipBin ?? 'pnpm'
  const args = input.knipBin
    ? ['--reporter', 'json']
    : ['exec', 'knip', '--reporter', 'json']
  const r = spawn(bin, args, input.workspace, input.timeoutMs)
  const findings: string[] = []
  if (r.status === 0) return { ran: true, findings, counts: {}, groupCount: 0 }

  // Parse structured output so the artifact carries file/symbol/type, not a blob.
  // knip's JSON is { issues: [ { file, exports: [...], files: [...], ... } ] } —
  // one group per file with typed arrays. An earlier version of this parser read
  // `issues` as flat {type, file, symbol} objects and therefore labelled every
  // single finding "issue", which made the report useless.
  const parsed = parseKnipJson(r.stdout)
  findings.push(...parsed.findings)
  if (parsed.groupCount === 0 && findings.length === 0) {
    findings.push(r.out.slice(0, 400))
  }
  return { ran: true, findings, counts: parsed.counts, groupCount: parsed.groupCount }
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
  // An explicit bin path counts only if it EXISTS. Otherwise a caller passing a
  // path that is not there produced a non-zero spawn with no output — a silent
  // FALSE FAIL of the architecture hard gate, which would block QA forever with
  // no evidence. A tool that is not present must skip, not fail.
  const hasDepcruiseBin =
    (input.depcruiseBin ? existsSync(input.depcruiseBin) : false) ||
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
    // `archRan` must be reported separately from `archOk`: a skipped gate is NOT
    // a clean architecture, and printing "clean" when the tool is absent is the
    // false-PASS this field exists to prevent.
    archRan: hasManifest && hasDepcruiseBin,
    archOk: arch.ok,
    archErrors: arch.errors,
    semgrepRan: sec.ran,
    semgrepFindings: sec.findings,
    knipRan: hygiene.ran,
    knipFindings: hygiene.findings,
    knipCounts: hygiene.counts,
    knipGroupCount: hygiene.groupCount,
    ok: arch.ok,
  }
}
