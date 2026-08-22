import { test } from 'node:test'
import assert from 'node:assert/strict'
import { DEADLINES, deadlineFor } from '../deadlines'
import { parseReSupermodel } from '../definitions/re-supermodel'
import type { DealWorkflowFacts } from '../facts'
import type { NodeDefinition } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// CRM-22 — deadline policy invariants.
//
// Acceptance criteria:
//   1. every implemented milestone has a canonical date source;
//   5. no deadline exists without an application source.
//
// Implemented milestones (inspection / financing / closing) must name a
// real DealWorkflowFacts key. Milestones WITHOUT a business date source
// (appraisal / title / tax-CRIM / funds / closing-documents) must stay
// unresolved — creating a source for them would be an artificial date.
// Every XML timer node must read its due date from a canonical fact
// (due-at-variable) — a timer never invents a date.
// ---------------------------------------------------------------------------

test('CRM-22: every implemented milestone has a canonical fact source', () => {
  // The factSource type (keyof DealWorkflowFacts | null) is enforced at
  // compile time; this runtime check pins the exact sources.
  const expected: Record<string, string> = {
    inspection: 'inspectionDeadline',
    financing: 'financingDeadline',
    closing: 'closingDate',
  }
  for (const [nodeId, factKey] of Object.entries(expected)) {
    const spec = DEADLINES[nodeId]
    assert.ok(spec, `expected a deadline policy for '${nodeId}'`)
    assert.equal(spec.factSource, factKey, `'${nodeId}' must source from '${factKey}'`)
  }
})

test('CRM-22: no artificial deadlines — unresolved milestones have no source', () => {
  for (const nodeId of ['appraisal', 'title_work', 'tax_clearance', 'funds_ready', 'closing_documents']) {
    const spec = DEADLINES[nodeId]
    assert.ok(spec, `expected a deadline policy for '${nodeId}'`)
    assert.equal(
      spec.factSource,
      null,
      `'${nodeId}' must stay unresolved — no canonical business date source exists`,
    )
    assert.ok(spec.note.length > 0, `'${nodeId}' must document why it is unresolved`)
  }
})

test('CRM-22: every XML timer node reads its due date from a canonical application fact', () => {
  const parsed = parseReSupermodel()
  const timerNodes = Object.values(parsed.graph.nodes).filter((n) => n.type === 'timer')
  assert.ok(timerNodes.length >= 3, 'closing + inspection + financing deadline timers')
  // The canonical date-bearing fact keys (the type-level guarantee is
  // keyof DealWorkflowFacts on DeadlineSpec.factSource; runtime pins values).
  const canonicalDateFacts = new Set([
    'closingDate',
    'closingDateScheduled',
    'inspectionDeadline',
    'inspectionDeadlineScheduled',
    'financingDeadline',
    'financingDeadlineScheduled',
  ])
  for (const node of timerNodes) {
    const dueVar = node.timer?.dueAtVariable
    assert.ok(dueVar, `timer '${node.id}' must use a due-at-variable (application fact)`)
    assert.ok(
      canonicalDateFacts.has(dueVar),
      `timer '${node.id}' due-at-variable '${dueVar}' must be a canonical DealWorkflowFacts key`,
    )
  }
})

test('CRM-22: each deadline timer is gated by a scheduled decision (never fires without a date)', () => {
  const parsed = parseReSupermodel()
  const gates: Array<{ timer: string; gate: string; fact: string }> = [
    { timer: 'closing_date_timer', gate: 'closing_deadline_applicable', fact: 'closingDateScheduled' },
    { timer: 'inspection_deadline_timer', gate: 'inspection_deadline_scheduled', fact: 'inspectionDeadlineScheduled' },
    { timer: 'financing_deadline_timer', gate: 'financing_deadline_scheduled', fact: 'financingDeadlineScheduled' },
  ]
  for (const { timer, gate, fact } of gates) {
    const g = parsed.graph.nodes[gate] as NodeDefinition & {
      decisions?: Array<{ condition: string; transition: string }>
    }
    assert.ok(g, `expected gate decision '${gate}'`)
    assert.equal(g.type, 'decision')
    assert.ok(
      g.decisions?.some((d) => d.condition === `${fact} == true` && d.transition === 'monitor'),
      `gate '${gate}' must route '${fact} == true' to the monitor`,
    )
    assert.ok(
      g.transitions?.some((t) => t.name === 'monitor' && t.to === timer),
      `gate '${gate}' must target timer '${timer}'`,
    )
  }
})

test('CRM-22: deadline monitors are OPTIONAL fork branches (join skips + cancels them)', () => {
  const parsed = parseReSupermodel()
  const fork = parsed.graph.nodes.fork_tracks
  assert.ok(fork)
  const deadlineBranches = fork.transitions?.filter((t) => t.name.endsWith('_deadline')) ?? []
  assert.equal(deadlineBranches.length, 2)
  for (const t of deadlineBranches) {
    assert.equal(t.required, false, `deadline monitor branch '${t.name}' must be optional`)
  }
})

test('CRM-22: every deadline escalation has an amend command and a proceed path', () => {
  const parsed = parseReSupermodel()
  const escalations: Array<{ task: string; command: string }> = [
    { task: 'inspection_deadline_escalation', command: 'set_inspection_deadline' },
    { task: 'financing_deadline_escalation', command: 'set_financing_deadline' },
    { task: 'closing_date_escalation', command: 'set_closing_date' },
  ]
  for (const { task, command } of escalations) {
    const t = parsed.graph.nodes[task]
    assert.ok(t, `expected escalation task '${task}'`)
    assert.equal(t.type, 'task')
    assert.ok(t.transitions?.some((x) => x.name === 'extend' && x.to === command))
    assert.ok(t.transitions?.some((x) => x.name === 'proceed'))
  }
})

test('CRM-22: deadlineFor returns the documented spec for known milestones', () => {
  assert.equal(deadlineFor('inspection').factSource, 'inspectionDeadline')
  assert.equal(deadlineFor('financing').factSource, 'financingDeadline')
  assert.equal(deadlineFor('closing').factSource, 'closingDate')
  assert.equal(deadlineFor('unknown-node').factSource, null)
})
