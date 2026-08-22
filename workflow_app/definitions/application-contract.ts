// ---------------------------------------------------------------------------
// Layer 4 — Application contract validation (ENG-14).
//
// This is the fourth of the four explicit workflow-definition validation
// layers. It is workflow_app-owned (application knowledge): every <command-node>
// in a definition references an application command (`commandType`) that the
// embedding application's port must be able to execute. An unknown command
// must fail HERE, at deployment, deterministically — never during live
// execution when the engine hands the request to the application port and the
// router has no case for it.
//
//   Layer 1  XML well-formedness       mini-xml.parseXml
//   Layer 2  Engine grammar            xml-parser
//   Layer 3  Generic graph semantics   xml/graph-validator
//   Layer 4  Application contract      THIS MODULE
//
// The routable inventory lives in `command-types.ts` (the canonical
// workflow_app command inventory, re-exporting the shared
// lib/commands/command-types.ts constants). This module is pure (no database),
// so it runs in the generic deploy pipeline and in tests without an
// environment.
//
// Scope: command-node `commandType` routability. General node-level checks are
// Layer 3's job (graph-validator); XML grammar checks are Layer 2's job.
// ---------------------------------------------------------------------------

import type { NodeDefinition, ProcessGraph } from '../../workflow_engine/lib/workflow/types'
import { assertCommandNodesRouted } from '../command-types'

export interface ApplicationContractResult {
  valid: boolean
  /** Actionable, deploy-time diagnostics. */
  errors: string[]
}

/**
 * Validate that every <command-node> in the graph references an application
 * command with a router case. Returns `{ valid: true, errors: [] }` when every
 * command-node is routable. Missing `commandType` is already an error in
 * Layer 3 (graph-validator); this layer only judges routability.
 */
export function validateApplicationContract(graph: ProcessGraph): ApplicationContractResult {
  const errors: string[] = []
  const commandTypes: string[] = []
  for (const node of Object.values(graph.nodes ?? {})) {
    if (
      node.type === 'command' &&
      typeof (node as NodeDefinition & { commandType?: string }).commandType === 'string'
    ) {
      const commandType = (node as NodeDefinition & { commandType: string }).commandType
      if (commandType.length > 0) commandTypes.push(commandType)
    }
  }

  const unrouted = assertCommandNodesRouted(commandTypes)
  for (const type of unrouted) {
    errors.push(
      `command node references application command '${type}' which has no router case — register it in the canonical command inventory (workflow_app/command-types.ts / lib/commands/register.ts) or remove the command-node`,
    )
  }
  return { valid: errors.length === 0, errors }
}
