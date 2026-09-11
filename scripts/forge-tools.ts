// ---------------------------------------------------------------------------
// forge-tools — the operator/agent-facing hook for the analyzer tools.
//
// The analyzer tools (V5-23..27) live inside lanes; this is the seam that lets a
// human or an agent interrogate and RUN them directly, without pretending a tool
// ran when it did not.
//
//   pnpm forge:tools                      wiring status for every tool
//   pnpm forge:tools --role smith         what is in force for ONE position
//   pnpm forge:tools --run                run the deterministic instruments here
//   pnpm forge:tools --run --workspace X  ...against a specific workspace
//
// Read-only apart from what the tools themselves do (the instruments never mutate).
// ---------------------------------------------------------------------------

import { existsSync } from 'node:fs'
import { resolve } from 'node:path'

import {
  FORGE_TOOL_CATALOG,
  FORGE_TOOL_IDS,
  modelForbiddenTools,
  resolveForgeToolPermissions,
  type ForgeToolRole,
} from '../workflow_app/forge/forge-tool-catalog'
import { runStaticGate } from '../workflow_app/forge/forge-static-gate'

const args = process.argv.slice(2)
const roleIndex = args.indexOf('--role')
const ROLE = roleIndex >= 0 ? (args[roleIndex + 1] as ForgeToolRole) : null
const RUN = args.includes('--run')
const wsIndex = args.indexOf('--workspace')
const WORKSPACE = resolve(wsIndex >= 0 ? (args[wsIndex + 1] ?? '.') : '.')

function printCatalog() {
  console.log('tool       class           wired  positions')
  for (const id of FORGE_TOOL_IDS) {
    const d = FORGE_TOOL_CATALOG[id]
    console.log(
      `${id.padEnd(10)} ${d.toolClass.padEnd(15)} ${(d.wired ? 'yes' : 'NO ').padEnd(6)} ${d.roles.join(' ')}`,
    )
  }
  console.log(`\nmodel-forbidden (never in a model catalog): ${modelForbiddenTools().join(', ')}`)
}

function printRole(role: ForgeToolRole) {
  const r = resolveForgeToolPermissions(role)
  console.log(`position: ${r.role}  (recomputed per transition; nothing retained)`)
  console.log(`model catalog : ${r.modelCatalog.join(', ') || '(none)'}`)
  console.log(`instruments   : ${r.instruments.join(', ') || '(none)'}`)
  for (const grant of r.grants) {
    console.log(
      `  grant ${grant.tool.padEnd(10)} write=${grant.canWrite ? 'yes' : 'no '}` +
        (grant.operations.length ? ` ops=${grant.operations.length}` : ''),
    )
  }
  for (const d of r.degradations) {
    console.log(`  degraded ${d.tool.padEnd(10)} (${d.reason}) -> ${d.fallback}`)
  }
}

async function runInstruments() {
  console.log(`running deterministic instruments against ${WORKSPACE}\n`)
  // Point at the primary checkout's binaries only when they are actually there —
  // passing a path that does not exist is what false-failed the gate before.
  const depcruiseBin = resolve(WORKSPACE, 'node_modules/.bin/depcruise')
  const knipBin = resolve(WORKSPACE, 'node_modules/.bin/knip')
  const gate = runStaticGate({
    workspace: WORKSPACE,
    depcruiseBin: existsSync(depcruiseBin) ? depcruiseBin : undefined,
    knipBin: existsSync(knipBin) ? knipBin : undefined,
  })

  console.log(
    gate.archRan
      ? `cruiser (dependency-cruiser) — HARD GATE: ${gate.archOk ? 'clean' : 'VIOLATIONS'}`
      : 'cruiser (dependency-cruiser) — HARD GATE: DID NOT RUN (tool not installed here)',
  )
  for (const error of gate.archErrors.slice(0, 20)) console.log(`  ${error}`)
  if (!gate.archOk && gate.archErrors.length > 20) {
    console.log(`  … ${gate.archErrors.length - 20} more`)
  }

  console.log(
    `\nsemgrep — informational: ${!gate.semgrepRan ? 'did not run (no .semgrep config)' : gate.semgrepFindings.length === 0 ? 'clean' : `${gate.semgrepFindings.length} finding(s)`}`,
  )
  for (const finding of gate.semgrepFindings.slice(0, 20)) console.log(`  ${finding}`)

  console.log(
    `\nknip — informational: ${!gate.knipRan ? 'did not run (no manifest or binary)' : gate.knipFindings.length === 0 ? 'clean' : `${gate.knipFindings.length} finding(s)`}`,
  )
  for (const finding of gate.knipFindings.slice(0, 20)) console.log(`  ${finding}`)

  console.log(
    `\noverall: ${!gate.archRan ? 'INCOMPLETE — the architecture gate did not run (tool not installed)' : gate.ok ? 'PASS' : 'FAIL'}`,
  )
}

async function main() {
  if (RUN) {
    await runInstruments()
    return
  }
  if (ROLE) {
    printRole(ROLE)
    return
  }
  printCatalog()
}

void main().then(() => process.exit(0)).catch((e) => { console.error(e); process.exit(1) })
