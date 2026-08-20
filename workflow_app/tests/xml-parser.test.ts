import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parseProcessDefinitionXml, XmlGrammarError } from '../xml/xml-parser'
import { parseReSupermodel } from '../definitions/re-supermodel'

const MINIMAL = `<?xml version="1.0" encoding="UTF-8"?>
<process-definition key="demo" version="1" name="Demo">
  <start-state id="start" label="Start">
    <transition name="go" to="work"/>
  </start-state>
  <task-node id="work" label="Do Work" responsibility="brokerage" priority="5" form-key="work-form">
    <transition name="done" to="end"/>
  </task-node>
  <end-state id="end" label="Done" outcome="completed"/>
</process-definition>
`

test('parses a minimal process into the expected ProcessGraph', () => {
  const parsed = parseProcessDefinitionXml(MINIMAL)
  assert.equal(parsed.key, 'demo')
  assert.equal(parsed.version, 1)
  assert.equal(parsed.name, 'Demo')
  assert.equal(parsed.graph.startNodeId, 'start')
  assert.deepEqual(Object.keys(parsed.graph.nodes).sort(), ['end', 'start', 'work'])

  const start = parsed.graph.nodes.start
  assert.equal(start.type, 'start')
  assert.equal(start.name, 'Start')
  assert.deepEqual(start.transitions, [{ name: 'go', to: 'work' }])

  const work = parsed.graph.nodes.work
  assert.equal(work.type, 'task')
  assert.equal(work.name, 'Do Work')
  assert.equal(work.responsibility, 'brokerage')
  assert.deepEqual(work.candidateGroups, ['brokerage'])
  assert.equal(work.priority, 5)
  assert.equal(work.formKey, 'work-form')

  const end = parsed.graph.nodes.end
  assert.equal(end.type, 'end')
  assert.equal(end.outcome, 'completed')
})

test('rejects duplicate node ids', () => {
  const xml = MINIMAL.replace(
    '<end-state id="end"',
    '<task-node id="work" label="dup" responsibility="brokerage"><transition name="x" to="end"/></task-node><end-state id="end"',
  )
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.ok(err instanceof XmlGrammarError)
    assert.match(err.message, /Duplicate node id 'work'/)
    return true
  })
})

test('rejects missing transition targets', () => {
  const xml = MINIMAL.replace('to="end"', 'to="missing_node"')
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.ok(err instanceof XmlGrammarError)
    assert.match(err.message, /targets missing node 'missing_node'/)
    return true
  })
})

test('rejects unsupported elements', () => {
  const xml = MINIMAL.replace(
    '</process-definition>',
    '<deal id="x"/></process-definition>',
  )
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.match(err.message, /Unsupported element <deal>/)
    return true
  })
})

test('rejects unsupported attributes', () => {
  const xml = MINIMAL.replace('responsibility="brokerage"', 'respnosibility="brokerage"')
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.match(err.message, /Unsupported attribute 'respnosibility'/)
    return true
  })
})

test('rejects more than one start-state', () => {
  const xml = MINIMAL.replace(
    '</process-definition>',
    '<start-state id="start2" label="Start 2"><transition name="go" to="end"/></start-state></process-definition>',
  )
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.match(err.message, /2 <start-state> elements/)
    return true
  })
})

test('rejects a missing start-state', () => {
  const xml = MINIMAL.replace(
    '<start-state id="start" label="Start">\n    <transition name="go" to="work"/>\n  </start-state>',
    '',
  )
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.match(err.message, /Missing <start-state>/)
    return true
  })
})

test('rejects invalid outcome on an end-state', () => {
  const xml = MINIMAL.replace('outcome="completed"', 'outcome="banana"')
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.match(err.message, /invalid outcome 'banana'/)
    return true
  })
})

test('rejects non-positive version and non-integer priority', () => {
  assert.throws(
    () => parseProcessDefinitionXml(MINIMAL.replace('version="1"', 'version="0"')),
    /version.*positive integer/,
  )
  assert.throws(
    () => parseProcessDefinitionXml(MINIMAL.replace('priority="5"', 'priority="high"')),
    /priority.*non-negative integer/,
  )
})

test('rejects text content inside nodes (definitions are attribute-driven)', () => {
  const xml = `<?xml version="1.0"?>
<process-definition key="d" version="1" name="D">
  <start-state id="start" label="Start"><transition name="go" to="end"/></start-state>
  <task-node id="work" label="Work">stray text</task-node>
  <end-state id="end" label="Done"/>
</process-definition>`
  assert.throws(() => parseProcessDefinitionXml(xml), (err: unknown) => {
    assert.match(err.message, /does not support text content/)
    return true
  })
})

test('preserves labels, descriptions, and display-order', () => {
  const xml = `<?xml version="1.0"?>
<process-definition key="d" version="1" name="D" description="The D">
  <display-order>
    <node ref="a"/>
    <node ref="end"/>
  </display-order>
  <start-state id="start" label="Start">
    <transition name="go" to="a"/>
  </start-state>
  <state id="a" label="A State" description="State A description">
    <transition name="go" to="end"/>
  </state>
  <end-state id="end" label="Done"/>
</process-definition>`
  const parsed = parseProcessDefinitionXml(xml)
  assert.equal(parsed.description, 'The D')
  assert.deepEqual(parsed.displayOrder, ['a', 'end'])
  assert.deepEqual(parsed.graph.displayOrder, ['a', 'end'])
  assert.equal(parsed.graph.nodes.a.name, 'A State')
  assert.equal(parsed.graph.nodes.a.description, 'State A description')
})

test('maps a decision with rules and transitions', () => {
  const xml = `<?xml version="1.0"?>
<process-definition key="d" version="1" name="D">
  <start-state id="start" label="Start">
    <transition name="go" to="decide"/>
  </start-state>
  <decision id="decide" label="Decide" refresh-facts="true">
    <on condition="flag == true" transition="yes"/>
    <transition name="no" to="end"/>
    <transition name="yes" to="work"/>
  </decision>
  <task-node id="work" label="Work" responsibility="brokerage">
    <transition name="done" to="end"/>
  </task-node>
  <end-state id="end" label="Done"/>
</process-definition>`
  const parsed = parseProcessDefinitionXml(xml)
  const decide = parsed.graph.nodes.decide
  assert.equal(decide.type, 'decision')
  assert.equal(decide.refreshFacts, true)
  assert.deepEqual(decide.decisions, [{ condition: 'flag == true', transition: 'yes' }])
  assert.deepEqual(decide.transitions, [
    { name: 'no', to: 'end' },
    { name: 'yes', to: 'work' },
  ])
})

test('maps timer, command, fork, join nodes', () => {
  const xml = `<?xml version="1.0"?>
<process-definition key="d" version="1" name="D">
  <start-state id="start" label="Start">
    <transition name="go" to="f"/>
  </start-state>
  <fork id="f" label="Fork">
    <transition name="a" to="timer" required="true"/>
    <transition name="b" to="cmd" required="false"/>
  </fork>
  <timer id="timer" label="Wait" due-at-variable="when" on-fire="resume">
    <transition name="resume" to="j"/>
  </timer>
  <command-node id="cmd" label="Do" command-type="app.do" transition="ok" responsibility="brokerage">
    <transition name="ok" to="j"/>
  </command-node>
  <join id="j" label="Join">
    <transition name="go" to="end"/>
  </join>
  <end-state id="end" label="Done"/>
</process-definition>`
  const parsed = parseProcessDefinitionXml(xml)
  assert.deepEqual(parsed.graph.nodes.f.transitions, [
    { name: 'a', to: 'timer', required: true },
    { name: 'b', to: 'cmd', required: false },
  ])
  assert.deepEqual(parsed.graph.nodes.timer.timer, {
    dueAtVariable: 'when',
    transition: 'resume',
  })
  assert.equal(parsed.graph.nodes.cmd.commandType, 'app.do')
  assert.equal(parsed.graph.nodes.cmd.transition, 'ok')
  assert.equal(parsed.graph.nodes.cmd.responsibility, 'brokerage')
})

test('RE_supermodel-v1.xml parses, validates, and has the expected spine', () => {
  const parsed = parseReSupermodel()
  assert.equal(parsed.key, 'RE_supermodel')
  assert.equal(parsed.version, 1)
  assert.equal(parsed.graph.startNodeId, 'start')

  const ids = Object.keys(parsed.graph.nodes)
  assert.ok(ids.length > 40, `expected a rich supermodel, got ${ids.length} nodes`)

  // Every transition target must exist (parser already enforces, but assert).
  for (const node of Object.values(parsed.graph.nodes)) {
    for (const t of node.transitions ?? []) {
      assert.ok(parsed.graph.nodes[t.to], `missing target ${t.to}`)
    }
  }

  // The simple path spine is present with stable node ids.
  for (const id of [
    'start',
    'offer_accepted',
    'pns_preparation',
    'pns_executed',
    'mark_under_contract',
    'under_contract',
    'fork_tracks',
    'title_work',
    'tax_clearance',
    'funds_ready',
    'closing_documents',
    'join_tracks',
    'closing_readiness_gate',
    'ready_to_close',
    'closing',
    'mark_closed',
    'closed_state',
    'post_closing',
    'recording',
    'post_closing_complete',
    'transaction_cancelled',
    'transaction_failed',
  ]) {
    assert.ok(parsed.graph.nodes[id], `missing node ${id}`)
  }
})

test('RE_supermodel responsibilities use the documented hint vocabulary', () => {
  const parsed = parseReSupermodel()
  const hints = new Set<string>()
  for (const node of Object.values(parsed.graph.nodes)) {
    if (node.responsibility) hints.add(node.responsibility)
  }
  const allowed = new Set([
    'brokerage',
    'buyer',
    'seller',
    'lender',
    'inspector',
    'appraiser',
    'notario',
    'title_company',
    'other_sme',
  ])
  for (const hint of hints) {
    assert.ok(allowed.has(hint), `responsibility hint '${hint}' is not in the documented set`)
  }
})
