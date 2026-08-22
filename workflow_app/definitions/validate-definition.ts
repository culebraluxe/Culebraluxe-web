// ---------------------------------------------------------------------------
// ENG-14 — Workflow Definition Validation / Static Analysis.
//
// Composes the four explicit validation layers into one deploy-time pipeline so
// invalid workflow definitions fail deterministically at deployment rather
// than during live execution:
//
//   Layer 1  XML well-formedness       mini-xml.parseXml          (XmlParseError)
//   Layer 2  Engine grammar            xml-parser                 (XmlGrammarError)
//   Layer 3  Generic graph semantics   xml/graph-validator        (GraphValidationResult)
//   Layer 4  Application contract      definitions/application-contract.ts
//
// The report carries actionable diagnostics grouped by layer (each flattened
// error is prefixed `[layer]`), plus the parsed definition when layers 1–2
// succeed, so the generic deploy pipeline never re-parses.
//
// The pipeline is generic: it accepts ANY file matching the workflow XML
// grammar. Layer 4 is workflow_app-specific by design — the workflow_app's
// application port can only execute commands that have a router case, so an
// unknown command-node must fail here, not at runtime.
// ---------------------------------------------------------------------------

import {
  parseProcessDefinitionXml,
  XmlGrammarError,
  type ParsedProcessDefinition,
} from '../xml'
import { XmlParseError } from '../xml/mini-xml'
import { validateProcessGraph, type GraphValidationResult } from '../xml/graph-validator'
import {
  validateApplicationContract,
  type ApplicationContractResult,
} from './application-contract'

export interface DefinitionValidationReport {
  valid: boolean
  /** Layer 1 — XML well-formedness (parser throws, bucketed). */
  xml: { errors: string[] }
  /** Layer 2 — engine grammar (parser throws, bucketed). */
  grammar: { errors: string[] }
  /** Layer 3 — generic graph semantics. */
  graph: GraphValidationResult
  /** Layer 4 — application contract (command routability). */
  application: ApplicationContractResult
  /** Flattened, layer-prefixed, actionable diagnostics. */
  errors: string[]
  warnings: string[]
  /** The parsed definition when layers 1–2 succeeded; null otherwise. */
  parsed: ParsedProcessDefinition | null
}

const EMPTY_GRAPH: GraphValidationResult = { valid: false, errors: [], warnings: [] }
const EMPTY_APPLICATION: ApplicationContractResult = { valid: false, errors: [] }

function withPrefix(prefix: string, messages: string[]): string[] {
  return messages.map((m) => `[${prefix}] ${m}`)
}

/** Run layers 3 + 4 against an already-parsed definition. */
export function validateParsedDefinition(
  parsed: ParsedProcessDefinition,
): Omit<DefinitionValidationReport, 'xml' | 'grammar' | 'parsed'> {
  const graph = validateProcessGraph(parsed.graph)
  const application = validateApplicationContract(parsed.graph)
  const errors = [
    ...withPrefix('graph', graph.errors),
    ...withPrefix('application', application.errors),
  ]
  return {
    valid: graph.valid && application.valid,
    graph,
    application,
    errors,
    warnings: graph.warnings,
  }
}

/**
 * Run ALL four validation layers against a workflow XML source. Never throws
 * for invalid input: every diagnostic is captured in the report. Throws only
 * for internal/plumbing errors that are not definition problems.
 */
export function validateWorkflowDefinitionXml(source: string): DefinitionValidationReport {
  const xml: { errors: string[] } = { errors: [] }
  const grammar: { errors: string[] } = { errors: [] }
  let parsed: ParsedProcessDefinition | null = null

  try {
    parsed = parseProcessDefinitionXml(source) // layers 1 + 2
  } catch (err) {
    if (err instanceof XmlParseError) {
      xml.errors.push(err.message)
    } else if (err instanceof XmlGrammarError) {
      grammar.errors.push(err.message)
    } else {
      throw err
    }
  }

  if (!parsed) {
    const errors = [
      ...withPrefix('xml', xml.errors),
      ...withPrefix('grammar', grammar.errors),
    ]
    return {
      valid: false,
      xml,
      grammar,
      graph: EMPTY_GRAPH,
      application: EMPTY_APPLICATION,
      errors,
      warnings: [],
      parsed: null,
    }
  }

  const layers = validateParsedDefinition(parsed)
  const errors = [
    ...withPrefix('xml', xml.errors),
    ...withPrefix('grammar', grammar.errors),
    ...layers.errors,
  ]
  return {
    valid: layers.valid && xml.errors.length === 0 && grammar.errors.length === 0,
    xml,
    grammar,
    graph: layers.graph,
    application: layers.application,
    errors,
    warnings: layers.warnings,
    parsed,
  }
}

/** Convenience: does the source pass all four layers? */
export function isWorkflowDefinitionValid(source: string): boolean {
  return validateWorkflowDefinitionXml(source).valid
}
