// OPS-11A — Live verification of the issue queue end-to-end against the DEV
// control plane (deterministic conditions, dedupe, bounded read model, sort,
// resolve). Read-mostly; the only writes are the issue rows themselves.
//
// Run: node --env-file=.env.local --import tsx scripts/verify-issues.mjs

import { reconcileIssues, getIssueQueue, resolveIssue } from '../db/issues'

let failures = 0
function check(name: string, ok: boolean, extra = '') {
  console.log(`${ok ? 'ok' : 'FAIL'} - ${name}${extra ? ' :: ' + extra : ''}`)
  if (!ok) failures++
}

async function main() {
  // 1. Deterministic generation is idempotent (running twice does not
  //    duplicate OPEN rows). `first` may create 0 if rows already exist (e.g.
  //    after seeding) — the real-materialization proof is the RED row in the
  //    open queue below.
  const first = await reconcileIssues()
  const second = await reconcileIssues()
  check(
    'reconcile idempotent (no duplicate OPEN)',
    second.created === 0,
    `created first=${first.created} second=${second.created}`,
  )

  // 2. Bounded read model, sorted RED→YELLOW→INFO then oldest-first.
  const open = await getIssueQueue({ scope: 'OPERATIONS_EXCEPTION', state: 'OPEN' })
  check('open queue bounded (<=50)', open.rows.length <= 50)
  check('queue has at least one row', open.rows.length > 0)
  check('queue total matches rows on one page', open.total >= open.rows.length)
  const red = open.rows.find((r) => r.severity === 'RED')
  check(
    'at least one real RED condition materialized (missing executed PS)',
    Boolean(red),
  )
  const severityRank = { RED: 0, YELLOW: 1, INFO: 2 } as const
  let sorted = true
  for (let i = 1; i < open.rows.length; i++) {
    const a = severityRank[open.rows[i - 1].severity]
    const b = severityRank[open.rows[i].severity]
    if (b < a) sorted = false
  }
  check('queue sorted by severity (RED first)', sorted)

  // 3. At most one OPEN per (type, domain_type, domain_id) — DB backstop.
  const seen = new Set<string>()
  let dedupe = true
  for (const r of open.rows) {
    const key = `${r.type}|${r.domainType}|${r.domainId}`
    if (seen.has(key)) dedupe = false
    seen.add(key)
  }
  check('no duplicate OPEN for same type+domain', dedupe)

  // 4. Each row carries a runbook + bounded facts (deal context resolvable).
  const withRunbook = open.rows.filter((r) => r.runbook && r.runbook.steps.length > 0)
  check('runbook present for all types', withRunbook.length === open.rows.length)
  check('RED issue resolves to a related deal', Boolean(red?.relatedDealId))

  // 5. Resolve an OPEN issue → leaves the open queue, appears in RESOLVED.
  if (red) {
    const ok = await resolveIssue(red.id)
    check('resolveIssue returns true for an OPEN issue', ok)
    const after = await getIssueQueue({ scope: 'OPERATIONS_EXCEPTION', state: 'OPEN' })
    check(
      'resolved issue leaves the open queue',
      !after.rows.some((r) => r.id === red.id),
    )
    const resolved = await getIssueQueue({
      scope: 'OPERATIONS_EXCEPTION',
      state: 'RESOLVED',
    })
    check(
      'resolved issue appears in the RESOLVED queue',
      resolved.rows.some((r) => r.id === red.id),
    )
    const again = await resolveIssue(red.id)
    check('resolveIssue idempotent (false on already-resolved)', again === false)
  }

  // 6. SUPPORT scope yields empty (all current types are operations).
  const support = await getIssueQueue({ scope: 'SUPPORT_EXCEPTION', state: 'OPEN' })
  check('SUPPORT scope is empty (reusable sibling surface ready)', support.rows.length === 0)

  console.log(failures === 0 ? '\nVERIFY PASSED' : `\nVERIFY FAILED (${failures})`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
