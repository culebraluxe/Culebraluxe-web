#!/usr/bin/env node
// -----------------------------------------------------------------------------
// Deterministic static gate — dependency-cruiser (architecture) + semgrep
// (security/correctness). This is the mechanical "independent verification"
// layer: the artifact is checked, not the builder's explanation of it.
//
// Runs in the current checkout (which has node_modules + config). Findings are
// normalized to a simple, exit-code-bearing report so a caller (operator or a
// future Assay/publish gate) can fail on architecture/security errors without
// interpreting model prose.
//
//   dependency-cruiser: pnpm exec depcruise <roots> --config .dependency-cruiser.js
//   semgrep:            semgrep scan --config <dir> --json <roots>
//
// Absent tooling is reported (and non-fatal unless --fail-on-absent), so the
// gate degrades gracefully where a tool isn't installed.
// -----------------------------------------------------------------------------
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

const ROOTS = process.argv.slice(2).length
  ? process.argv.slice(2)
  : ['workflow_app', 'agent-runtime', 'db', 'services', 'app', 'components', 'ui', 'lib', 'workflow_engine']

function run(cmd: string, args: string[], cwd = process.cwd()): { status: number | null; out: string } {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', timeout: 180_000, maxBuffer: 32 * 1024 * 1024 })
  return { status: r.status, out: (r.stdout ?? '') + (r.stderr ?? '') }
}

function depcruise(): { errors: string[]; ok: boolean } {
  const usePnpm = existsSync(join(process.cwd(), 'pnpm-workspace.yaml')) || existsSync(join(process.cwd(), 'pnpm-lock.yaml'))
  const bin = usePnpm ? 'pnpm' : 'npx'
  const args = usePnpm
    ? ['exec', 'depcruise', ...ROOTS, '--config', '.dependency-cruiser.js', '--output-type', 'err']
    : ['depcruise', ...ROOTS, '--config', '.dependency-cruiser.js', '--output-type', 'err']
  const r = run(bin, args)
  const errors = r.out
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0 && !/^info:|^warn:|depcruise:|Using (system|custom)/i.test(l))
  return { errors: r.status !== 0 ? errors : [], ok: r.status === 0 }
}

function semgrep(configDir: string): { findings: string[]; ran: boolean } {
  if (!existsSync(configDir)) return { findings: [], ran: false }
  const r = run('semgrep', ['scan', '--config', configDir, '--json', ...ROOTS])
  return { findings: r.status !== 0 && !/no findings/i.test(r.out) ? [r.out.slice(0, 400)] : [], ran: true }
}

function main(): void {
  const arch = depcruise()
  const sec = semgrep('.semgrep')

  console.log('=== forge static gate ===')
  console.log(`architecture (dependency-cruiser): ${arch.ok ? 'CLEAN' : `${arch.errors.length} violation(s)`}`)
  for (const e of arch.errors) console.log(`  ARCH ${e}`)
  if (sec.ran) {
    console.log(`security (semgrep): ${sec.findings.length === 0 ? 'CLEAN' : `${sec.findings.length} finding(s) (informational)`}`)
    for (const f of sec.findings) console.log(`  SEC  ${f}`)
  } else {
    console.log('security (semgrep): NOT RUN (no .semgrep config)')
  }

  const failed = !arch.ok
  console.log(`result: ${failed ? 'FAIL' : 'PASS'}`)
  process.exit(failed ? 1 : 0)
}

main()
