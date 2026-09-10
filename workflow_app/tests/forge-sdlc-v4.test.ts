import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  FORGE_SDLC_VERSION,
  forgeSdlcXmlSource,
  parseForgeSdlc,
  parseForgeSdlcV4,
} from '../definitions/forge-sdlc'

// Scope C activation anchor, extended by ENG-FORGE batch rollout (v5): the ACTIVE
// definition is v5. v5 mirrors v4 (FAST lane + Architect review park, v3 behavior
// preserved) and adds the batch-sliced deployment deferral branch, which lets a
// batch story complete with its deployment DEFERRED instead of being classified as
// a deployment failure.

test('active definition is v6, parses/validates, and carries the batch release deferral', () => {
  assert.equal(FORGE_SDLC_VERSION, 6)
  assert.doesNotThrow(() => parseForgeSdlc())
  const graph = parseForgeSdlc().graph
  const nodes = Object.keys(graph.nodes)
  // v4 behavior preserved: the FAST lane is still present.
  for (const id of ['fast_lane_entry', 'fast_smith', 'fast_qa_verify', 'fast_publish']) {
    assert.ok(nodes.includes(id), `missing ${id}`)
  }
  // v5's addition: deployment_result accepts a deferred deployment and completes.
  // Asserted against the definition SOURCE (shape-independent): the branch must come
  // before the `deploymentSucceeded == false` failure branch, or a deferred story
  // would still be classified as a deployment failure.
  const source = forgeSdlcXmlSource()
  const deferredBranch = source.indexOf('condition="deploymentDeferred == true" transition="deferred"')
  const failureBranch = source.indexOf('condition="deploymentSucceeded == false" transition="fail"')
  assert.ok(deferredBranch > 0, 'deployment_result must branch on deploymentDeferred')
  assert.ok(failureBranch > 0, 'the deployment failure branch must still exist')
  assert.ok(deferredBranch < failureBranch, 'the deferred branch must be evaluated BEFORE the failure branch')
  assert.match(source, /<transition name="deferred" to="complete"\/>/, 'deferred must complete the story')

  // v6's addition: a batch story holds the WHOLE release tail at qa_result, so
  // nothing is published (publishing main IS the production trigger).
  const qaFail = source.indexOf('condition="qaPassed == false" transition="fail"')
  const releaseDeferred = source.indexOf('condition="releaseDeferred == true" transition="deferred"')
  const qaPass = source.indexOf('condition="qaPassed == true" transition="pass"')
  assert.ok(releaseDeferred > 0, 'qa_result must branch on releaseDeferred')
  assert.ok(qaFail < releaseDeferred, 'a QA failure must still route to repair BEFORE the deferral is considered')
  assert.ok(releaseDeferred < qaPass, 'the deferral must be considered BEFORE the normal release pass')
  // Architect review park present (reuses hold; no new end state).
  assert.ok(nodes.includes('architect_review'))
  // v4 loader agrees with the active loader.
  assert.equal(Object.keys(parseForgeSdlcV4().graph.nodes).length, nodes.length)
})