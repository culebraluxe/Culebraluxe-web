// Read-only: print the Forge run-effectiveness scorecard (the first Maestro pull).
// Usage: pnpm forge:scorecard [windowDays]
import { computeForgeScorecard } from '../workflow_app/forge/forge-scorecard'
import { sql } from '../db/client'

const pct = (v: number | null) => (v === null ? 'n/a' : `${(v * 100).toFixed(1)}%`)
const n = (v: number | null) => (v === null ? 'n/a' : String(v))

async function main() {
  const days = Number(process.argv[2] ?? 30)
  const s = await computeForgeScorecard(sql, days)
  const o = s.outcomes

  console.log(`\nFORGE RUN EFFECTIVENESS — last ${s.windowDays} days`)
  console.log(`  runs                ${o.runs}`)
  console.log(`  complete            ${o.complete}  (${pct(o.cleanCompletionRate)})`)
  console.log(`  hold                ${o.hold}  (${pct(o.holdRate)})`)
  console.log(`  failed              ${o.failed}  (${pct(o.failureRate)})`)
  console.log(`  interrupted         ${o.interrupted}`)
  console.log(`  avg run minutes     ${n(o.avgMinutes)}`)
  console.log(`  tests passed/failed ${o.testsPassed}/${o.testsFailed}`)

  console.log('\nROUTE MIX (lead_decision)')
  for (const route of s.routes) console.log(`  ${route.route.padEnd(10)} ${route.runs}`)

  console.log('\nSPLIT HEALTH')
  console.log(`  split runs                    ${s.splitHealth.splitRuns}`)
  console.log(`  children declared             ${s.splitHealth.children}`)
  console.log(`  smith items with candidate SHA ${s.splitHealth.withCandidateSha}`)

  console.log('\nBY ROLE (work items)')
  console.log(`  ${'role'.padEnd(12)}${'items'.padStart(7)}${'err'.padStart(8)}${'attempts'.padStart(10)}${'min'.padStart(7)}`)
  for (const r of s.roles) {
    console.log(
      `  ${r.role.padEnd(12)}${String(r.items).padStart(7)}${pct(r.errorRate).padStart(8)}${n(r.avgAttempts).padStart(10)}${n(r.avgMinutes).padStart(7)}`,
    )
  }

  console.log('\nOBSERVER LAYER (worker execution)')
  if (s.observerEvents.length === 0) {
    console.log('  none yet — the hook records on the next split-child attempt')
  } else {
    for (const e of s.observerEvents) console.log(`  ${e.eventType.padEnd(18)} ${e.events}`)
  }

  console.log(`\nTELEMETRY  ${s.telemetry.note}`)
  console.log(`  runs with tokens: ${s.telemetry.runsWithTokens}   runs with cost: ${s.telemetry.runsWithCost}`)
  console.log('')
}

void main().then(() => process.exit(0)).catch((error) => {
  console.error(error)
  process.exit(1)
})
