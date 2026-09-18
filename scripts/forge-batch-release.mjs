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
//   node --import tsx --env-file=.env.local --env-file=.env.local scripts/forge-batch-release.mjs               # list
//   node --import tsx --env-file=.env.local --env-file=.env.local scripts/forge-batch-release.mjs --batch 1     # one slice
// ---------------------------------------------------------------------------

import { forgeDb, forgeDbTargetForUrl } from '../db/forge-db.ts'
import { sliceForBatch, sliceOf } from '../workflow_app/forge/forge-batch-slice.ts'

const argv = process.argv.slice(2)
const only = argv.includes('--batch') ? Number(argv[argv.indexOf('--batch') + 1]) : null
const target = (argv.includes('--target') ? argv[argv.indexOf('--target') + 1] : 'prod') === 'dev' ? 'dev' : 'prod'
const url = target === 'dev' ? process.env.DATABASE_URL_DEV : process.env.DATABASE_URL_PROD

const pool = forgeDb.forTarget(forgeDbTargetForUrl(url))
// WHY THIS IS NOT SCOPED TO ONE ROLLOUT: the query used to read
// `where s.id like 'PROJECTS-WORKSPACE-%'`, so a story that deferred its deployment from any OTHER
// batch was invisible here — measured 2026-09-18, when ENG-FORGE-SCOPE-OWN-CHANGES-01 recorded
// `deployment_deferred_to_batch = 92` and this tool reported "0 story(ies)". A deferral that the
// release view cannot see is exactly the deployment nobody remembers at sprint release, which is the
// failure the deferral record exists to prevent. The slice logic already ignores rows with no
// deferral (`sliceOf` -> null), so the filter belongs there, not in the query. Use --batch to narrow.
const rows = await pool.query(
  `select s.id, s.batch, s.status, s.batch_deploy,
          e.qa_passed, e.published_sha, e.deployed_sha, e.deployment_deferred_to_batch,
          e.deployment_receipt, e.production_verified
     from storyboard_story s
     left join forge_workflow_evidence e on e.story_id = s.id
    where e.deployment_deferred_to_batch is not null
    order by s.batch nulls last, s.id`,
)

const stories = rows.rows.map((r) => ({
  id: r.id,
  batch: r.batch,
  status: r.status,
  qaPassed: r.qa_passed,
  publishedSha: r.published_sha,
  deployedSha: r.deployed_sha,
  productionVerified: r.production_verified,
  deploymentReceipt: r.deployment_receipt,
  deploymentDeferredToBatch: r.deployment_deferred_to_batch,
}))

// The slice is the RECORDED deferral target; a row with no deferral is in no
// slice and is never guessed into one.
const batches = [...new Set(stories.map(sliceOf).filter((b) => b !== null))].sort((a, b) => a - b)

console.log(`batch release view (${target.toUpperCase()}) — deferred deployments are NOT deployments\n`)
for (const batch of batches) {
  if (only !== null && batch !== only) continue
  console.log(`BATCH ${batch}`)
  for (const entry of sliceForBatch(stories, batch)) {
    const published = entry.publishedSha ? `  published=${String(entry.publishedSha).slice(0, 8)}` : ''
    console.log(`  ${entry.id}  ${entry.state}${published}`)
  }
  console.log('')
}
const deferred = stories.filter((r) => sliceOf(r) !== null && !r.deploymentReceipt && !r.productionVerified)
console.log(`${deferred.length} story(ies) carry a deferred deployment; none of them claims production verification.`)
await pool.end()
