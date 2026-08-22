// ---------------------------------------------------------------------------
// CRM-14G — Workflow command inventory (authoritative, dependency-free).
//
// Single source of truth for which command types workflow_app routes and which
// of those are referenced by <command-node> elements in RE_supermodel-v1.xml.
// This module imports nothing from the database layer (the shared command-type
// constants in lib/commands/command-types.ts are pure strings), so the guard
// tests run in the main unit suite and the RE_supermodel loader can assert
// routability at deploy time without an environment.
//
// CRM-14J: the command-type identifiers are now the canonical layer's
// constants (lib/commands/command-types.ts) re-exported here, so the workflow
// inventory and the canonical command registry can never drift apart. Routing
// itself now flows through the canonical CommandDispatcher
// (workflow_app/command-router.ts is a translation seam, not a rule holder).
//
// Command inventory for the CURRENT XML — complete; remaining work is
// reconciliation, not new command implementation:
//
//   XML command-nodes (exactly three; each has a registered handler):
//     mark_under_contract -> deal.set_stage_under_contract -> db/deal-stage.ts
//     mark_closed         -> deal.set_stage_closed         -> db/deal-stage.ts
//     set_closing_date    -> deal.set_closing_date         -> db/deal-closing-date.ts
//
//   Routed but NOT referenced by any XML command-node (application-only):
//     deal.set_financing_type -> db/deal-financing.ts — financing is READ as
//       the financingApplicable fact (facts.ts); this model never sets
//       financing via a workflow command. Routed for application use; this is
//       NOT a workflow command gap.
//     deal.set_appraisal_required -> db/deal-appraisal.ts — appraisal
//       applicability is READ as the appraisalApplicable fact (facts.ts) from
//       the canonical deal.appraisal_required column; resolved by this
//       explicit application command (CRM-19), never by a workflow command.
//     deal.set_lender_clear_to_close -> db/deal-lender-clearance.ts — lender
//       clear-to-close is READ as the lenderClearToClose fact (facts.ts) from
//       the canonical deal.lender_clear_to_close column; recorded by this
//       explicit application command (CRM-20), never by a workflow command.
//       Lender provider behavior is never modeled inside the engine.
//     offer.accept, task.create, task.complete, task.cancel — application /
//       engine-integration commands outside the workflow XML.
//
//   There is NO command deal.set_closing_readiness_verified: closing readiness
//   is the human task-node `closing_readiness` (resolved -> ready_to_close),
//   gated by the closing_readiness_gate decision on closingConfirmationRequired
//   (Stories 135/136 removed the boolean as the wrong semantic shape).
//
// Boundary: command routing is workflow_app (mapping engine request -> canonical
// CommandEnvelope -> canonical CommandDispatcher). Canonical business
// validation lives in db/deal-*.ts. The engine never knows command names.
// ---------------------------------------------------------------------------

import {
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_CLOSING_DATE,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_LENDER_CLEAR_TO_CLOSE,
  DEAL_SET_STAGE_CLOSED,
  DEAL_SET_STAGE_UNDER_CONTRACT,
  OFFER_ACCEPT,
  TASK_CANCEL,
  TASK_COMPLETE,
  TASK_CREATE,
} from '../lib/commands/command-types'

// Re-export the canonical identifiers (public API unchanged from CRM-14G).
export {
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_CLOSING_DATE,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_LENDER_CLEAR_TO_CLOSE,
  DEAL_SET_STAGE_CLOSED,
  DEAL_SET_STAGE_UNDER_CONTRACT,
  OFFER_ACCEPT,
  TASK_CANCEL,
  TASK_COMPLETE,
  TASK_CREATE,
}

/** Command-node types referenced by RE_supermodel-v1.xml (source: the XML). */
export const XML_COMMAND_NODE_TYPES: ReadonlySet<string> = new Set([
  DEAL_SET_STAGE_UNDER_CONTRACT,
  DEAL_SET_STAGE_CLOSED,
  DEAL_SET_CLOSING_DATE,
])

/**
 * Routed command types NOT referenced by any XML command-node. Each is routed
 * for application/engine integration, never for a workflow command-node.
 */
export const ROUTED_BUT_UNREFERENCED_COMMAND_TYPES: ReadonlySet<string> = new Set([
  OFFER_ACCEPT,
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_LENDER_CLEAR_TO_CLOSE,
  TASK_CREATE,
  TASK_COMPLETE,
  TASK_CANCEL,
])

/** Every command type with a registered handler in the canonical registry. */
export const ROUTED_COMMAND_TYPES: ReadonlySet<string> = new Set([
  ...XML_COMMAND_NODE_TYPES,
  ...ROUTED_BUT_UNREFERENCED_COMMAND_TYPES,
])

/**
 * Deploy/test guard: returns the command-node types that have NO router case.
 * An empty array means every command-node in the definition is routable. Used
 * by parseReSupermodel() (deploy-time) and tests/command-inventory.test.ts, so
 * a command-node added to the XML without a router case fails immediately.
 */
export function assertCommandNodesRouted(commandNodeTypes: Iterable<string>): string[] {
  const unrouted: string[] = []
  for (const type of new Set(commandNodeTypes)) {
    if (!ROUTED_COMMAND_TYPES.has(type)) unrouted.push(type)
  }
  return unrouted.sort()
}
