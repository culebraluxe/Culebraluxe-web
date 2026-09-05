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
import { ROUTED_COMMAND_TYPES } from '../command-types'

// ---------------------------------------------------------------------------
// Layer 4 — Application contract validation (ENG-14).
//
// The generic workflow pipeline accepts ANY definition; Layer 4 is the
// embedding-application gate: every <command-node> must reference an
// application command the engine's ApplicationPort can actually execute.
//
// ENG-FORGE-V9 — the application-contract is now DOMAIN-FORKED. The engine
// hosts two clean models (RE_supermodel for real estate, FORGE_SDLC for Forge)
// and the "rest of the pieces" fork A/B: each definition validates its
// command-nodes against ITS OWN command inventory, never the other domain's.
//
//   A) Real estate (default, unchanged): workflow_app/command-types.ts
//      ROUTED_COMMAND_TYPES — RE business commands (deal.*, offer.*, task.*).
//   B) Forge: workflow_app/forge-command-types.ts FORGE_ROUTED_COMMAND_TYPES —
//      Forge commands only. FORGE_SDLC is NEVER validated against the RE
//      registry, so an RE command name inside FORGE_SDLC fails closed here.
//
// Default inventory is RE so RE_supermodel behavior is byte-for-byte unchanged;
// the FORGE_SDLC loader (forge-sdlc.ts) passes the Forge inventory explicitly.
// ---------------------------------------------------------------------------

export type CommandRoutability = (commandType: string) => boolean

/** A) Real-estate routability: is this command in the RE canonical registry? */
export const reCommandIsRouted: CommandRoutability = (commandType) =>
  ROUTED_COMMAND_TYPES.has(commandType)

export interface ApplicationContractResult {
  valid: boolean
  /** Actionable, deploy-time diagnostics. */
  errors: string[]
}

/**
 * Validate that every <command-node> in the graph references a command the
 * given domain inventory can route. `isRouted` defaults to the real-estate
 * inventory so RE_supermodel deployment/tests are unchanged.
 */
export function validateApplicationContract(
  graph: ProcessGraph,
  isRouted: CommandRoutability = reCommandIsRouted,
): ApplicationContractResult {
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
    // dynamic-fork branches execute a per-branch command-node (ENG-FORGE-V9), so
    // its branch-command-type must be routed in this domain's inventory too.
    if (
      node.type === 'dynamic-fork' &&
      typeof (node as NodeDefinition & { branchCommandType?: string }).branchCommandType === 'string'
    ) {
      const branchCommandType = (node as NodeDefinition & { branchCommandType: string })
        .branchCommandType
      if (branchCommandType.length > 0) commandTypes.push(branchCommandType)
    }
  }

  const unrouted = [...new Set(commandTypes)]
    .filter((type) => !isRouted(type))
    .sort()
  for (const type of unrouted) {
    errors.push(
      `command node references application command '${type}' which has no router case in this domain's command inventory (RE: workflow_app/command-types.ts; Forge: workflow_app/forge-command-types.ts) — register it in the correct domain inventory + canonical handler, or remove the command-node`,
    )
  }
  return { valid: errors.length === 0, errors }
}
