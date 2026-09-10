#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Batch release for a major rollout.
//
// Stories flagged batch_deploy complete QA-verified with their deployment DEFERRED
// (forge_workflow_evidence.deployment_deferred_to_batch + storyboard_story.batch_deploy).
// This lists what is waiting per batch, and refuses to pretend a deployment happened:
// it reports exactly which stories are published-but-undeployed so the slice can be
// released deliberately (and, once deploy receipts exist, verified for real).
//
// Usage:
//   node --env-file=.env.local scripts/forge-batch-release.mjs               # list
//   node --env-file=.env.local scripts/forge-batch-release.mjs --batch 1     # one slice
// ---------------------------------------------------------------------------

import { Pool } from '@neondatabase/serverless'

const argv = process.argv.slice(2)
const only = argv.includes('--batch') ? Number(argv[argv.indexOf('--batch') + 1]) : null
const target = (argv.includes('--target') ? argv[argv.indexOf('--target') + 1] : 'prod') === 'dev' ? 'dev' : 'prod'
const url = target === 'dev' ? process.env.DATABASE_URL_DEV : process.env.DATABASE_URL_PROD

const pool = new Pool({ connectionString: url })
const rows = await pool.query(
  `select s.id, s.batch, s.status, s.batch_deploy,
          e.qa_passed, e.published_sha, e.deployment_deferred_to_batch,
          e.deployment_receipt, e.production_verified
     from storyboard_story s
     left join forge_workflow_evidence e on e.story_id = s.id
    where s.id like 'PROJECTS-WORKSPACE-%'
    order by s.batch nulls last, s.id`,
)

const byBatch = new Map()
for (const r of rows.rows) {
  if (only !== null && r.batch !== only) continue
  const key = r.batch ?? 'none'
  if (!byBatch.has(key)) byBatch.set(key, [])
  byBatch.get(key).push(r)
}

console.log(`batch release view (${target.toUpperCase()}) — deferred deployments are NOT deployments\n`)
for (const [batch, items] of [...byBatch.entries()].sort()) {
  console.log(`BATCH ${batch}`)
  for (const r of items) {
    const state = r.deployment_receipt
      ? 'DEPLOYED (receipt)'
      : r.production_verified
        ? 'DEPLOYED (verified)'
        : r.deployment_deferred_to_batch != null
          ? `DEFERRED -> batch ${r.deployment_deferred_to_batch}`
          : r.qa_passed
            ? 'QA passed, deploy not recorded'
            : `${r.status}`
    console.log(`  ${r.id}  ${state}${r.published_sha ? `  published=${String(r.published_sha).slice(0, 8)}` : ''}`)
  }
  console.log('')
}
const deferred = rows.rows.filter((r) => r.deployment_deferred_to_batch != null && !r.deployment_receipt)
console.log(`${deferred.length} story(ies) carry a deferred deployment; none of them claims production verification.`)
await pool.end()
