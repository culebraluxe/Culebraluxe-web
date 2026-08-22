import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  parseReSupermodel,
  reSupermodelXmlSource,
} from '../definitions/re-supermodel'
import {
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_LENDER_CLEAR_TO_CLOSE,
  ROUTED_COMMAND_TYPES,
  ROUTED_BUT_UNREFERENCED_COMMAND_TYPES,
  XML_COMMAND_NODE_TYPES,
  assertCommandNodesRouted,
} from '../command-types'
import type { NodeDefinition } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// CRM-14G — Workflow command inventory completion.
//
// Guards the RE_supermodel-v1.xml command-nodes against the command router: a
// command-node added to the XML without a router case fails here (and at deploy
// time via parseReSupermodel()). No database, no packages.
// ---------------------------------------------------------------------------

function xmlCommandNodeTypes(): string[] {
  const parsed = parseReSupermodel()
  return Object.values(parsed.graph.nodes)
    .filter(
      (n): n is NodeDefinition & { commandType: string } =>
        n.type === 'command' && typeof n.commandType === 'string',
    )
    .map((n) => n.commandType)
    .sort()
}

test('CRM-14G: every command-node in RE_supermodel-v1.xml maps to a router case', () => {
  const nodeTypes = xmlCommandNodeTypes()

  // The XML references exactly the three documented command-node types.
  assert.deepEqual(
    nodeTypes,
    [...XML_COMMAND_NODE_TYPES].sort(),
    'XML command-node types must match the documented inventory exactly',
  )

  // Each of them has a router case (guard primitive used at deploy time too).
  assert.deepEqual(assertCommandNodesRouted(nodeTypes), [])
  for (const t of nodeTypes) {
    assert.ok(ROUTED_COMMAND_TYPES.has(t), `XML command-node type '${t}' is not routed`)
  }
})

test('CRM-14G: deal.set_closing_readiness_verified is NOT a command-node (readiness is a human task)', () => {
  const parsed = parseReSupermodel()
  const commandNodeTypes = Object.values(parsed.graph.nodes)
    .filter((n) => n.type === 'command')
    .map((n) => n.commandType!)

  assert.ok(
    !commandNodeTypes.includes('deal.set_closing_readiness_verified'),
    'no command-node may reference deal.set_closing_readiness_verified',
  )
  assert.ok(
    !ROUTED_COMMAND_TYPES.has('deal.set_closing_readiness_verified'),
    'deal.set_closing_readiness_verified must not be in the routed inventory',
  )

  // Readiness is the human task-node closing_readiness, gated by a decision on
  // closingConfirmationRequired.
  assert.equal(
    parsed.graph.nodes.closing_readiness?.type,
    'task',
    'closing_readiness must be a human task-node',
  )
  assert.equal(
    parsed.graph.nodes.closing_readiness_gate?.type,
    'decision',
    'closing_readiness_gate must be a decision',
  )
  const gate = parsed.graph.nodes.closing_readiness_gate as NodeDefinition & {
    decisions?: Array<{ condition: string; transition: string }>
  }
  assert.ok(
    gate.decisions?.some((d) => d.condition === 'closingConfirmationRequired == true'),
    'closing_readiness_gate must decide on closingConfirmationRequired',
  )
})

test('CRM-21: closing-document readiness is a derived fact consumed by a gate — never a command or a bare human checkmark', () => {
  const parsed = parseReSupermodel()

  // No command-node may represent closing-document readiness (it is derived
  // from the packet catalog + transaction_document signed lineage, never set).
  const commandNodeTypes = Object.values(parsed.graph.nodes)
    .filter((n) => n.type === 'command')
    .map((n) => n.commandType!)
  assert.ok(
    !commandNodeTypes.some((t) => t.includes('closing_document')),
    'no command-node may exist for closing-document readiness',
  )

  // The closing_documents_gate is a decision on the derived fact, with an
  // explicit pending task for the not-ready path (blocks readiness).
  const gate = parsed.graph.nodes.closing_documents_gate as NodeDefinition & {
    decisions?: Array<{ condition: string; transition: string }>
  }
  assert.equal(
    parsed.graph.nodes.closing_documents_pending?.type,
    'task',
    'closing_documents_pending must be a human task-node',
  )
  assert.equal(gate?.type, 'decision', 'closing_documents_gate must be a decision')
  assert.ok(
    gate.decisions?.some((d) => d.condition === 'closingDocumentsReady == true'),
    'closing_documents_gate must decide on the derived closingDocumentsReady fact',
  )
  assert.ok(
    gate.transitions?.some((t) => t.name === 'pending' && t.to === 'closing_documents_pending'),
    'closing_documents_gate must route not-ready to the pending task',
  )
})

test('CRM-14G: deal.set_financing_type is routed but unreferenced by the XML (application-only)', () => {
  const parsed = parseReSupermodel()
  const xmlCommandNodeTypes = Object.values(parsed.graph.nodes)
    .filter((n) => n.type === 'command')
    .map((n) => n.commandType!)

  assert.ok(
    ROUTED_COMMAND_TYPES.has(DEAL_SET_FINANCING_TYPE),
    'deal.set_financing_type stays routed for application use (db/deal-financing.ts)',
  )
  assert.ok(
    !XML_COMMAND_NODE_TYPES.has(DEAL_SET_FINANCING_TYPE),
    'deal.set_financing_type must not be documented as an XML command-node',
  )
  assert.ok(
    !xmlCommandNodeTypes.includes(DEAL_SET_FINANCING_TYPE),
    'the XML must not reference deal.set_financing_type',
  )
})

test('CRM-19: deal.set_appraisal_required is routed but unreferenced by the XML (application-only)', () => {
  const parsed = parseReSupermodel()
  const xmlCommandNodeTypes = Object.values(parsed.graph.nodes)
    .filter((n) => n.type === 'command')
    .map((n) => n.commandType!)

  assert.ok(
    ROUTED_COMMAND_TYPES.has(DEAL_SET_APPRAISAL_REQUIRED),
    'deal.set_appraisal_required stays routed for application use (db/deal-appraisal.ts)',
  )
  assert.ok(
    !XML_COMMAND_NODE_TYPES.has(DEAL_SET_APPRAISAL_REQUIRED),
    'deal.set_appraisal_required must not be documented as an XML command-node',
  )
  assert.ok(
    !xmlCommandNodeTypes.includes(DEAL_SET_APPRAISAL_REQUIRED),
    'the XML must not reference deal.set_appraisal_required',
  )
})

test('CRM-20: deal.set_lender_clear_to_close is routed but unreferenced by the XML (application-only)', () => {
  const parsed = parseReSupermodel()
  const xmlCommandNodeTypes = Object.values(parsed.graph.nodes)
    .filter((n) => n.type === 'command')
    .map((n) => n.commandType!)

  assert.ok(
    ROUTED_COMMAND_TYPES.has(DEAL_SET_LENDER_CLEAR_TO_CLOSE),
    'deal.set_lender_clear_to_close stays routed for application use (db/deal-lender-clearance.ts)',
  )
  assert.ok(
    !XML_COMMAND_NODE_TYPES.has(DEAL_SET_LENDER_CLEAR_TO_CLOSE),
    'deal.set_lender_clear_to_close must not be documented as an XML command-node',
  )
  assert.ok(
    !xmlCommandNodeTypes.includes(DEAL_SET_LENDER_CLEAR_TO_CLOSE),
    'the XML must not reference deal.set_lender_clear_to_close',
  )
})

test('CRM-14G: routed inventory partitions exactly into XML command-nodes and documented outside-XML commands', () => {
  const expected = new Set([
    ...XML_COMMAND_NODE_TYPES,
    ...ROUTED_BUT_UNREFERENCED_COMMAND_TYPES,
  ])
  assert.deepEqual(
    [...ROUTED_COMMAND_TYPES].sort(),
    [...expected].sort(),
    'ROUTED_COMMAND_TYPES must be exactly the XML command-nodes plus the documented application-only commands',
  )
  for (const t of ROUTED_BUT_UNREFERENCED_COMMAND_TYPES) {
    assert.ok(
      !XML_COMMAND_NODE_TYPES.has(t),
      `'${t}' cannot be both an XML command-node and unreferenced`,
    )
  }
})

test('CRM-14G: the stale XML header markers are gone', () => {
  const source = reSupermodelXmlSource()
  assert.ok(
    !source.includes('(NEW — application gap)'),
    'the stale "(NEW — application gap)" markers must be removed from the header',
  )
  assert.ok(
    !source.includes('closingReadinessVerified'),
    'the removed closingReadinessVerified boolean must not appear in the XML header',
  )
  assert.ok(
    source.includes(DEAL_SET_FINANCING_TYPE),
    'the header must document the routed-but-unreferenced classification',
  )
})
