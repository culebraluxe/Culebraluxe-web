#!/usr/bin/env node
// ---------------------------------------------------------------------------
// ⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
// The TypeScript engine and its libraries were deleted in the 2026-09 Rust port, so this
// file cannot load: the modules it imports from are gone. It is kept as REFERENCE ONLY,
// so the behaviour it describes can be translated into Rust when that behaviour is wanted.
// Reviving it in place is forbidden — see AGENTS.md ("legacy/ is out of scope") and
// docs/agent/BROKEN-TS-INVENTORY.md for the priority list and each capability's Rust home.
// ---------------------------------------------------------------------------
// Deterministic static gate CLI — runs runStaticGate against the current checkout
// and prints a normalized report with an exit code (architecture is the hard gate).
import { runStaticGate } from '@/legacy/workflow_app/forge/forge-static-gate'

const roots = process.argv.slice(2)
const result = runStaticGate({ workspace: process.cwd(), roots: roots.length ? roots : undefined })

console.log('=== forge static gate ===')
console.log(
  `architecture (dependency-cruiser): ${result.archOk ? 'CLEAN' : `${result.archErrors.length} violation(s)`}`,
)
for (const e of result.archErrors) console.log(`  ARCH ${e}`)
if (result.semgrepRan) {
  console.log(
    `security (semgrep): ${result.semgrepFindings.length === 0 ? 'CLEAN' : `${result.semgrepFindings.length} finding(s) (informational)`}`,
  )
  for (const f of result.semgrepFindings) console.log(`  SEC  ${f}`)
} else {
  console.log('security (semgrep): NOT RUN (no .semgrep config)')
}

console.log(`result: ${result.ok ? 'PASS' : 'FAIL'}`)
process.exit(result.ok ? 0 : 1)
