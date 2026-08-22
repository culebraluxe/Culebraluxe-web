// ---------------------------------------------------------------------------
// CRM-14G — Workflow command inventory (authoritative, dependency-free).
//
// Single source of truth for which command types workflow_app routes and which
// of those are referenced by <command-node> elements in RE_supermodel-v1.xml.
// This module imports nothing from the database layer, so the guard tests run
// in the main unit suite and the RE_supermodel loader can assert routability at
// deploy time without an environment.
//
// Command inventory for the CURRENT XML — complete; remaining work is
// reconciliation, not new command implementation:
//
//   XML command-nodes (exactly three; each has a router case):
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
// app service). Canonical business validation lives in db/deal-*.ts. The engine
// never knows command names.
// ---------------------------------------------------------------------------

/** Command-node types referenced by RE_supermodel-v1.xml (source: the XML). */
export const XML_COMMAND_NODE_TYPES: ReadonlySet<string> = new Set([
  'deal.set_stage_under_contract',
  'deal.set_stage_closed',
  'deal.set_closing_date',
])

/** Routed command type for application-only financing writes. */
export const DEAL_SET_FINANCING_TYPE = 'deal.set_financing_type'

/** Routed command type for application-only appraisal-applicability writes. */
export const DEAL_SET_APPRAISAL_REQUIRED = 'deal.set_appraisal_required'

/** Routed command type for application-only lender clear-to-close writes. */
export const DEAL_SET_LENDER_CLEAR_TO_CLOSE = 'deal.set_lender_clear_to_close'

/**
 * Routed command types NOT referenced by any XML command-node. Each is routed
 * for application/engine integration, never for a workflow command-node.
 */
export const ROUTED_BUT_UNREFERENCED_COMMAND_TYPES: ReadonlySet<string> = new Set([
  'offer.accept',
  DEAL_SET_FINANCING_TYPE,
  DEAL_SET_APPRAISAL_REQUIRED,
  DEAL_SET_LENDER_CLEAR_TO_CLOSE,
  'task.create',
  'task.complete',
  'task.cancel',
])

/** Every command type with a router case in workflow_app/command-router.ts. */
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
