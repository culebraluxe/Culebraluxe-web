#!/usr/bin/env node
// Deterministic static gate CLI — runs runStaticGate against the current checkout
// and prints a normalized report with an exit code (architecture is the hard gate).
import { runStaticGate } from '../workflow_app/forge/forge-static-gate'

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
