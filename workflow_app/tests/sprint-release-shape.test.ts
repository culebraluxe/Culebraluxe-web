import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { deriveReleaseEvidence } from '../forge/agent-runtime-role-runner'
import { recordForgeBatchReleaseReceipt } from '../../db/forge-workflow-evidence'

// ---------------------------------------------------------------------------
// THE SPRINT SHAPE: no build or deploy per story, one identity per batch (2026-09-19).
//
// The operator's model, in his words: "we dont do a build per story we do a build and deploy on SPRINT which is
// comprised of many batches of many stories". A story in a batch records a DEFERRAL instead of a deployment, and
// the release identity belongs to the batch — one sha stamped across every story in it — so a per-story
// expectation anywhere in the chain is the wrong granularity no matter which role holds it.
//
// The fences below prove the parts that are environment-free (a refusal, a stamped receipt) and state the parts
// that are structural. They are explicit about which is which.
// ---------------------------------------------------------------------------

const REPO_ROOT = new URL('../../', import.meta.url)

test('sprint shape: the deploy stage REFUSES to release a story that requires a deployment, with no receipt', async () => {
  // Fail-closed, and the whole point of the deferral: a story that owes a deploy cannot be released by the
  // deploy stage on the strength of proofs alone.
  const released = await deriveReleaseEvidence({
    nodeId: 'deploy',
    deploymentRequired: true,
    publishedSha: null,
    deploymentReceipt: null,
    deployedSha: null,
    productionVerificationReceipt: null,
    productionVerifiedSha: null,
    proofs: [],
    cwd: process.cwd(),
  })
  assert.equal(released, null, 'a deploy-owing story must not be released without a deployment receipt')
})

test('sprint shape: a batch story records the deferral, so no per-story deploy is ever demanded', () => {
  // The runtime consequence lives in the port wiring: a batch-sliced story supplies
  // `deploymentDeferredToBatch` AND `deploymentRequired: false`, which is what makes the refusal above the
  // correct answer for it rather than a HOLD. Comments stripped, because the comment names the fields.
  const runner = readFileSync(new URL('workflow_app/forge/agent-runtime-role-runner.ts', REPO_ROOT), 'utf8')
  const code = runner.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '')
  assert.match(
    code,
    /resolvedStory\.batchDeploy\s*\n?\s*\?\s*\{[^}]*deploymentDeferredToBatch[^}]*deploymentRequired:\s*false/,
    'a batch story must record the deferral and drop its deployment requirement',
  )
})

test('sprint shape: the release is ONE receipt per batch, stamped across every story, and refused without it', async () => {
  // The identity granularity, proven where it is environment-free: the writer refuses to stamp a batch release
  // that has no derived receipt, and when it does stamp one it stamps the SAME sha for every story in the batch,
  // scoped to the stories deferred to that batch.
  await assert.rejects(
    () => recordForgeBatchReleaseReceipt(null),
    /refusing to write a batch release without a derived receipt/,
    'a batch release with no derived receipt must be refused',
  )

  const source = readFileSync(new URL('db/forge-workflow-evidence.ts', REPO_ROOT), 'utf8')
  assert.match(source, /batch_released_sha = \$\{receipt\.releasedSha\}/, 'one sha per batch, not per story')
  assert.match(source, /for \(const storyId of receipt\.storyIds\)/, 'stamped across every story in the batch')
  assert.match(
    source,
    /deployment_deferred_to_batch = \$\{receipt\.batch\}/,
    'and only onto the stories deferred to that batch',
  )
})
