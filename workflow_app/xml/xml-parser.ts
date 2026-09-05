// ---------------------------------------------------------------------------
// XML → ProcessGraph grammar parser.
//
// Consumes the generic tree produced by `mini-xml.ts` and maps it onto the
// generic workflow engine's runtime representation (`ProcessGraph` /
// `NodeDefinition` / `TransitionDefinition`). This module is generic: it
// imports nothing from CulebraLuxe domain code and contains no domain policy.
//
// Grammar (Story 113 / 116 / 117):
//
//   process-definition (key, version, name, description)
//     display-order?                     -> ordered node ids for presentation
//       node (ref)                       -> references a node id
//     start-state (id, label, description)          exactly one required
//     state (id, label, description)
//     task-node (id, label, description, responsibility, priority, form-key)
//     command-node (id, label, description, command-type, transition, responsibility)
//     decision (id, label, description, refresh-facts)
//       on (condition, transition)       -> decision rules
//       transition*                      -> available transitions
//     fork (id, label, description)
//       transition*                      -> branch transitions (required attr)
//     join (id, label, description)
//       transition*
//     timer (id, label, description, due-at, due-at-variable, on-fire)
//       transition*
//     end-state (id, label, description, outcome)
//
//   transition (name, to, condition, required)   (child of most node types)
//
// The XML node id IS the workflow state identity (Story 116). `label` becomes
// the presentation `name`; `description` is preserved. `responsibility` is
// preserved as node metadata and mirrored into candidateGroups on task nodes
// (Story 117). No second state enum/mapping is created.
//
// The parser rejects, with a descriptive error:
//   - duplicate node ids
//   - transition targets that do not exist
//   - unknown elements / attributes / non-empty text content
//   - anything other than exactly one start-state
//   - invalid attribute values (version, priority, refresh-facts, outcome, ...)
// ---------------------------------------------------------------------------

import type {
  NodeDefinition,
  ProcessGraph,
  ProcessOutcome,
  TransitionDefinition,
} from '../../workflow_engine/lib/workflow/types'
import { isXmlElement, parseXml, XmlElement, XmlParseError } from './mini-xml'

export interface ParsedProcessDefinition {
  /** Stable definition key (the deployment identity). */
  key: string
  version: number
  name: string
  description: string | null
  /** Ordered node ids for portal timeline presentation (optional). */
  displayOrder: string[]
  graph: ProcessGraph
}

export class XmlGrammarError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'XmlGrammarError'
  }
}

const TRUE = 'true'
const FALSE = 'false'

const BOOLEAN_ATTRS = new Set([TRUE, FALSE])

const OUTCOMES: ReadonlySet<string> = new Set<ProcessOutcome>([
  'completed',
  'cancelled',
  'failed',
  'conflict',
])

// ---------------------------------------------------------------------------
// Allowed element / attribute surfaces (strict "unsupported XML rejected
// explicitly" policy — unknown elements/attributes fail deterministically).
// ---------------------------------------------------------------------------

const ROOT_ATTRS = new Set(['key', 'version', 'name', 'description'])
const COMMON_ATTRS = new Set(['id', 'label', 'description'])
const TRANSITION_ATTRS = new Set(['name', 'to', 'condition', 'required'])
const ON_ATTRS = new Set(['condition', 'transition'])

const ELEMENT_ALLOWED_ATTRS: Record<string, ReadonlySet<string>> = {
  'process-definition': ROOT_ATTRS,
  'display-order': new Set([]),
  node: new Set(['ref']),
  'start-state': COMMON_ATTRS,
  state: COMMON_ATTRS,
  'task-node': new Set([
    ...COMMON_ATTRS,
    'responsibility',
    'priority',
    'form-key',
  ]),
  'command-node': new Set([
    ...COMMON_ATTRS,
    'command-type',
    'transition',
    'responsibility',
  ]),
  decision: new Set([...COMMON_ATTRS, 'refresh-facts']),
  fork: COMMON_ATTRS,
  join: COMMON_ATTRS,
  timer: new Set([
    ...COMMON_ATTRS,
    'due-at',
    'due-at-variable',
    'on-fire',
  ]),
  'end-state': new Set([...COMMON_ATTRS, 'outcome']),
  'dynamic-fork': new Set([
    ...COMMON_ATTRS,
    'responsibility',
    'count-variable',
    'plan-variable',
    'branch-command-type',
    'join',
    'minimum',
    'maximum',
  ]),
  transition: TRANSITION_ATTRS,
  on: ON_ATTRS,
}

/** Which node elements accept <transition> children. */
const TRANSITION_CAPABLE = new Set([
  'start-state',
  'state',
  'task-node',
  'command-node',
  'decision',
  'fork',
  'join',
  'timer',
])

const NODE_ELEMENTS = new Set([
  'start-state',
  'state',
  'task-node',
  'command-node',
  'decision',
  'fork',
  'join',
  'timer',
  'end-state',
  'dynamic-fork',
])

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

export function parseProcessDefinitionXml(source: string): ParsedProcessDefinition {
  const root = parseXml(source)
  if (root.name !== 'process-definition') {
    throw new XmlGrammarError(
      `Root element must be <process-definition>, found <${root.name}>`,
    )
  }
  assertAllowedAttrs(root, 'process-definition')

  const key = requiredAttr(root, 'key')
  const version = parsePositiveInt(root, 'version')
  const name = requiredAttr(root, 'name')
  const description = root.attributes.description ?? null

  const displayOrder: string[] = []
  const nodes: Record<string, NodeDefinition> = {}
  let startCount = 0

  for (const child of root.children) {
    if (!isXmlElement(child)) {
      if (child.text.trim().length > 0) {
        throw new XmlGrammarError(
          'Unexpected text content inside <process-definition>: definitions are attribute-driven; text is not supported',
        )
      }
      continue
    }

    if (child.name === 'display-order') {
      assertAllowedAttrs(child, 'display-order')
      for (const entry of child.children) {
        if (!isXmlElement(entry)) {
          throw new XmlGrammarError('Unexpected text inside <display-order>')
        }
        if (entry.name !== 'node') {
          throw new XmlGrammarError(
            `Unsupported element <${entry.name}> inside <display-order> (expected <node>)`,
          )
        }
        assertAllowedAttrs(entry, 'node')
        const ref = requiredAttr(entry, 'ref')
        displayOrder.push(ref)
      }
      continue
    }

    if (!NODE_ELEMENTS.has(child.name)) {
      throw new XmlGrammarError(
        `Unsupported element <${child.name}> inside <process-definition>`,
      )
    }

    const node = parseNodeElement(child)
    if (nodes[node.id] !== undefined) {
      throw new XmlGrammarError(`Duplicate node id '${node.id}'`)
    }
    nodes[node.id] = node
    if (node.type === 'start') startCount++
  }

  if (startCount === 0) {
    throw new XmlGrammarError('Missing <start-state>: exactly one start-state is required')
  }
  if (startCount > 1) {
    throw new XmlGrammarError(`Found ${startCount} <start-state> elements; exactly one is required`)
  }

  const startNodeId = Object.keys(nodes).find((id) => nodes[id].type === 'start')!
  const graph: ProcessGraph = {
    startNodeId,
    nodes,
    ...(displayOrder.length > 0 ? { displayOrder } : {}),
  }

  // Resolve every transition target against the collected node ids.
  for (const node of Object.values(nodes)) {
    for (const t of node.transitions ?? []) {
      if (nodes[t.to] === undefined) {
        throw new XmlGrammarError(
          `Transition '${t.name}' on node '${node.id}' targets missing node '${t.to}'`,
        )
      }
    }
  }

  // Every display-order ref must exist.
  for (const ref of displayOrder) {
    if (nodes[ref] === undefined) {
      throw new XmlGrammarError(`display-order references missing node '${ref}'`)
    }
  }

  return { key, version, name, description, displayOrder, graph }
}

// ---------------------------------------------------------------------------
// Node mapping
// ---------------------------------------------------------------------------

function parseNodeElement(el: XmlElement): NodeDefinition {
  assertAllowedAttrs(el, el.name)
  const id = requiredAttr(el, 'id')
  const label = el.attributes.label ?? id
  const description = el.attributes.description ?? undefined
  const responsibility = el.attributes.responsibility ?? undefined
  const transitions = collectTransitions(el)

  switch (el.name) {
    case 'start-state':
      return { id, type: 'start', name: label, description, transitions }
    case 'state':
      return { id, type: 'state', name: label, description, transitions }
    case 'end-state': {
      const outcomeRaw = el.attributes.outcome ?? 'completed'
      if (!OUTCOMES.has(outcomeRaw)) {
        throw new XmlGrammarError(
          `<end-state id="${id}"> has invalid outcome '${outcomeRaw}' (expected ${[...OUTCOMES].join(' | ')})`,
        )
      }
      return {
        id,
        type: 'end',
        name: label,
        description,
        outcome: outcomeRaw as ProcessOutcome,
      }
    }
    case 'task-node': {
      const priorityRaw = el.attributes.priority
      const priority = priorityRaw !== undefined ? parseNonNegativeInt(el, 'priority') : undefined
      const formKey = el.attributes['form-key']
      return {
        id,
        type: 'task',
        name: label,
        description,
        responsibility,
        candidateGroups: responsibility ? [responsibility] : undefined,
        priority,
        formKey,
        transitions,
      }
    }
    case 'command-node': {
      const commandType = requiredAttr(el, 'command-type')
      const successTransition = el.attributes.transition
      return {
        id,
        type: 'command',
        name: label,
        description,
        responsibility,
        commandType,
        transition: successTransition,
        transitions,
      }
    }
    case 'decision': {
      const refreshRaw = el.attributes['refresh-facts']
      if (refreshRaw !== undefined && !BOOLEAN_ATTRS.has(refreshRaw)) {
        throw new XmlGrammarError(
          `<decision id="${id}"> has invalid refresh-facts '${refreshRaw}' (expected 'true' | 'false')`,
        )
      }
      const refreshFacts = refreshRaw !== undefined ? refreshRaw === TRUE : undefined
      const decisions: { condition: string; transition: string }[] = []
      for (const child of el.children) {
        if (!isXmlElement(child)) {
          if (child.text.trim().length > 0) {
            throw new XmlGrammarError(
              `<decision id="${id}"> does not support text content`,
            )
          }
          continue
        }
        if (child.name === 'transition') continue // consumed by collectTransitions
        if (child.name !== 'on') {
          throw new XmlGrammarError(
            `Unsupported element <${child.name}> inside <decision id="${id}"> (expected <on> or <transition>)`,
          )
        }
        assertAllowedAttrs(child, 'on')
        decisions.push({
          condition: requiredAttr(child, 'condition'),
          transition: requiredAttr(child, 'transition'),
        })
      }
      return {
        id,
        type: 'decision',
        name: label,
        description,
        decisions,
        refreshFacts,
        transitions,
      }
    }
    case 'fork':
      return { id, type: 'fork', name: label, description, transitions }
    case 'join':
      return { id, type: 'join', name: label, description, transitions }
    case 'timer': {
      const dueAt = el.attributes['due-at']
      const dueAtVariable = el.attributes['due-at-variable']
      const onFire = el.attributes['on-fire']
      return {
        id,
        type: 'timer',
        name: label,
        description,
        timer: {
          ...(dueAt !== undefined ? { dueAt } : {}),
          ...(dueAtVariable !== undefined ? { dueAtVariable } : {}),
          ...(onFire !== undefined ? { transition: onFire } : {}),
        },
        transitions,
      }
    }
    case 'dynamic-fork': {
      const countVariable = requiredAttr(el, 'count-variable')
      const branchCommandType = requiredAttr(el, 'branch-command-type')
      const joinTarget = requiredAttr(el, 'join')
      const planVariable = el.attributes['plan-variable'] ?? undefined
      const minimum = parseOptionalNonNegativeInt(el, 'minimum', 2)
      const maximum = parseOptionalNonNegativeInt(el, 'maximum', 8)
      if (minimum > maximum) {
        throw new XmlGrammarError(
          `<dynamic-fork id="${id}"> minimum (${minimum}) must not exceed maximum (${maximum})`,
        )
      }
      return {
        id,
        type: 'dynamic-fork',
        name: label,
        description,
        responsibility,
        countVariable,
        ...(planVariable !== undefined ? { planVariable } : {}),
        branchCommandType,
        join: joinTarget,
        minimum,
        maximum,
      }
    }
    default:
      throw new XmlGrammarError(`Unsupported node element <${el.name}>`)
  }
}

function collectTransitions(el: XmlElement): TransitionDefinition[] | undefined {
  const transitions: TransitionDefinition[] = []
  for (const child of el.children) {
    if (!isXmlElement(child)) {
      if (child.text.trim().length > 0) {
        throw new XmlGrammarError(
          `<${el.name} id="${el.attributes.id ?? ''}"> does not support text content`,
        )
      }
      continue
    }
    if (child.name === 'on') continue // decision rules are handled separately
    if (child.name === 'transition') {
      if (!TRANSITION_CAPABLE.has(el.name)) {
        throw new XmlGrammarError(
          `<${el.name}> does not support <transition> children`,
        )
      }
      assertAllowedAttrs(child, 'transition')
      const name = requiredAttr(child, 'name')
      const to = requiredAttr(child, 'to')
      const condition = child.attributes.condition
      const requiredRaw = child.attributes.required
      if (requiredRaw !== undefined && !BOOLEAN_ATTRS.has(requiredRaw)) {
        throw new XmlGrammarError(
          `<transition name="${name}"> has invalid required '${requiredRaw}' (expected 'true' | 'false')`,
        )
      }
      const required = requiredRaw !== undefined ? requiredRaw === TRUE : undefined
      transitions.push({
        name,
        to,
        ...(condition !== undefined ? { condition } : {}),
        ...(required !== undefined ? { required } : {}),
      })
      continue
    }
    throw new XmlGrammarError(
      `Unsupported element <${child.name}> inside <${el.name}>`,
    )
  }
  return transitions.length > 0 ? transitions : undefined
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function assertAllowedAttrs(el: XmlElement, elementName: string): void {
  const allowed = ELEMENT_ALLOWED_ATTRS[elementName]
  if (!allowed) {
    throw new XmlGrammarError(`No attribute allowlist registered for <${elementName}>`)
  }
  for (const attr of Object.keys(el.attributes)) {
    if (!allowed.has(attr)) {
      throw new XmlGrammarError(
        `Unsupported attribute '${attr}' on <${elementName}${el.attributes.id ? ` id="${el.attributes.id}"` : ''}>`,
      )
    }
  }
}

function requiredAttr(el: XmlElement, name: string): string {
  const value = el.attributes[name]
  if (value === undefined || value.length === 0) {
    throw new XmlGrammarError(`Missing required attribute '${name}' on <${el.name}>`)
  }
  return value
}

function parsePositiveInt(el: XmlElement, attr: string): number {
  const raw = requiredAttr(el, attr)
  const value = Number(raw)
  if (!Number.isInteger(value) || value <= 0) {
    throw new XmlGrammarError(
      `<${el.name}> attribute '${attr}' must be a positive integer, got '${raw}'`,
    )
  }
  return value
}

function parseNonNegativeInt(el: XmlElement, attr: string): number {
  const raw = requiredAttr(el, attr)
  const value = Number(raw)
  if (!Number.isInteger(value) || value < 0) {
    throw new XmlGrammarError(
      `<${el.name}> attribute '${attr}' must be a non-negative integer, got '${raw}'`,
    )
  }
  return value
}

/** Optional non-negative integer attribute with a default when absent. */
function parseOptionalNonNegativeInt(
  el: XmlElement,
  attr: string,
  fallback: number,
): number {
  const raw = el.attributes[attr]
  if (raw === undefined || raw === '') return fallback
  const value = Number(raw)
  if (!Number.isInteger(value) || value < 0) {
    throw new XmlGrammarError(
      `<${el.name}> attribute '${attr}' must be a non-negative integer, got '${raw}'`,
    )
  }
  return value
}

export { XmlParseError }
