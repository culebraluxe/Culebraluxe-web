import assert from 'node:assert/strict'
import test from 'node:test'

import { resolveLane } from '../../agent-runtime/lane-policy'
import { parseForgeSdlc } from '../definitions/forge-sdlc'

function hotfixRouteTargets(): string[] {
  const decision = parseForgeSdlc().graph.nodes['hotfix_architecture_check']
  assert.ok(decision, 'hotfix_architecture_check must exist')
  assert.equal(decision.type, 'decision')
  const byName = new Map((decision.transitions ?? []).map((t) => [t.name, t.to]))
  return (decision.decisions ?? []).map((rule) => {
    const target = byName.get(rule.transition)
    assert.ok(target, `hotfix branch '${rule.condition}' must resolve to a transition target`)
    return target
  })
}

test('HOTFIX reaches lead_pre with the Architect contract the lane policy requires', () => {
  const nodes = parseForgeSdlc().graph.nodes
  assert.deepEqual([...new Set(hotfixRouteTargets())], ['architect'])
  assert.ok(
    (nodes['architect']?.transitions ?? []).some((t) => t.to === 'architect_review'),
    'Architect must hand off to the review gate',
  )
  assert.ok(
    (nodes['architect_review']?.transitions ?? []).some((t) => t.to === 'lead_pre'),
    'the review gate must proceed to lead_pre',
  )
  const lead = resolveLane({ lane: 'lead', session: { hasArchitectBrief: true } })
  assert.equal(lead.ok, true, 'Lead launches with the Architect brief the route produces')
})

test('HOTFIX route and lane policy cannot diverge', () => {
  const refused = resolveLane({ lane: 'lead', session: { hasArchitectBrief: false } })
  assert.equal(refused.ok, false)
  if (!refused.ok) assert.equal(refused.code, 'missing-architect-brief')
  assert.ok(
    hotfixRouteTargets().every((target) => target !== 'lead_pre'),
    'a hotfix branch reaches lead_pre only after the Architect, never directly',
  )
})
