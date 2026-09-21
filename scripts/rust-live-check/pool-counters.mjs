// Pool counters around a real page load. This is the check that distinguishes "cold pool" from "slow database".
import { apiBase, internalKey, devIdentity, devDb, median } from './_env.mjs'

const key = internalKey()
const identity = await devIdentity()
const rounds = Number(process.argv[2] ?? 5)

const counters = async () => {
  const response = await fetch(`${apiBase}/v1/diagnostics/db`, { headers: { 'x-culebra-internal-key': key } })
  const payload = await response.json()
  return payload.value ?? payload
}

const before = await counters()
console.log(`target: ${before.target} (must be dev unless you meant otherwise)`)

const latencies = []
for (let i = 0; i < rounds; i += 1) {
  const started = Date.now()
  const response = await fetch(`${apiBase}/v1/clients?page=1&pageSize=25`, {
    headers: {
      'x-culebra-internal-key': key,
      'x-culebra-auth-provider': identity.provider,
      'x-culebra-auth-sub': identity.provider_subject,
    },
  })
  await response.text()
  latencies.push(Date.now() - started)
}

const after = await counters()
console.log(
  `page: median=${median(latencies)}ms min=${Math.min(...latencies)}ms max=${Math.max(...latencies)}ms (${rounds} rounds)`,
)
console.log(
  `pool: checkouts ${before.checkouts} -> ${after.checkouts} (+${after.checkouts - before.checkouts}), ` +
    `connections opened +${after.connectionsOpened - before.connectionsOpened}, ` +
    `idle probes +${after.idleProbes - before.idleProbes}, reuse=${after.connectionReuseRate}`,
)
console.log(
  `      per request: ~${((after.checkouts - before.checkouts) / rounds).toFixed(1)} checkouts. ` +
    `Identity is cached, so repeated requests should need 2 (page + evidence).`,
)

const db = await devDb()
const recent = await db.query(
  `select kind, level, count(*)::int as n from app_error
    where created_at > now() - interval '30 minutes' group by 1,2 order by 3 desc limit 6`,
)
await db.end()
console.log(recent.rowCount ? 'errors (30m):' : 'errors (30m): none')
for (const row of recent.rows) console.log(`  ${row.kind} ${row.level} x${row.n}`)
