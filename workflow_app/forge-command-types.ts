// ---------------------------------------------------------------------------
// ENG-FORGE-V9 — Forge command inventory (the "B" fork of Layer 4).
//
// The workflow engine hosts two clean models with separate command domains:
//
//   A) Real estate  -> workflow_app/command-types.ts  (deal.*, offer.*, task.*)
//   B) Forge        -> THIS module                     (forge.* only)
//
// FORGE_SDLC command-nodes are validated against THIS inventory by the FORGE
// loader (forge-sdlc.ts), NEVER against the RE registry. An RE command name
// placed inside FORGE_SDLC fails closed here, and a forge.* command can never
// silently ride RE's router.
//
// v1 (FORGE_SDLC-v1.xml) carries ZERO command-nodes, so this set is empty for
// now — matching V7. Each future forge.* command-node (FORGE_SDLC-v2) must gain
// a type here PLUS a Forge-owned canonical handler + router case before Layer 4
// will pass, exactly as the V7 packet's "router case + canonical handler first"
// contract requires.
//
// This module imports nothing from the database or the RE registry.
// ---------------------------------------------------------------------------

export const FORGE_COMMAND_NAMESPACE = 'forge'

// --- Forge state/app commands (Stage 3, application-side) -----------------
export const FORGE_STORY_MARK_HOLD = 'forge.story.hold'
export const FORGE_STORY_MARK_COMPLETE = 'forge.story.complete'
export const FORGE_STORY_MARK_IN_PROGRESS = 'forge.story.in_progress'
export const FORGE_RUN_APPEND_DETAIL = 'forge.run.detail'

// --- Authoritative role-execution command surface (FORGE_SDLC-v1 superset) --
export const FORGE_RUN_SCOUT = 'forge.run_scout'
export const FORGE_RUN_DIAGNOSIS = 'forge.run_diagnosis'
export const FORGE_RUN_ARCHITECT = 'forge.run_architect'
export const FORGE_RUN_LEAD_PRE = 'forge.run_lead_pre'
export const FORGE_RUN_LEAD_IMPLEMENT = 'forge.run_lead_implement'
export const FORGE_RUN_SMITH = 'forge.run_smith'
export const FORGE_RUN_SMITH_SPLIT = 'forge.run_smith_split'
export const FORGE_RUN_LEAD_POST = 'forge.run_lead_post'
export const FORGE_RUN_QA_REVIEW = 'forge.run_qa_review'
export const FORGE_RUN_QA_VERIFY = 'forge.run_qa_verify'
export const FORGE_CLASSIFY_FAILURE = 'forge.classify_failure'
export const FORGE_RUN_SMITH_REPAIR = 'forge.run_smith_repair'
export const FORGE_RUN_ARCHITECT_REPAIR = 'forge.run_architect_repair'
export const FORGE_RUN_DEVOPS_REPAIR = 'forge.run_devops_repair'
export const FORGE_PUBLISH_CANDIDATE = 'forge.publish_candidate'
export const FORGE_MIGRATE_DEV = 'forge.migrate_dev'
export const FORGE_VERIFY_DEV_MIGRATION = 'forge.verify_dev_migration'
export const FORGE_MIGRATE_PROD = 'forge.migrate_prod'
export const FORGE_VERIFY_PROD_MIGRATION = 'forge.verify_prod_migration'
export const FORGE_REFRESH_DERIVED_MODELS = 'forge.refresh_derived_models'
export const FORGE_VERIFY_DERIVED_MODELS = 'forge.verify_derived_models'
export const FORGE_DEPLOY = 'forge.deploy'
export const FORGE_VERIFY_PRODUCTION = 'forge.verify_production'

/** Command-node types referenced by FORGE_SDLC XML (filled as the XML is wired). */
export const FORGE_XML_COMMAND_NODE_TYPES: ReadonlySet<string> = new Set([])

/**
 * Every Forge command with a registered handler (or routing surface) in the
 * Forge command domain. Routing flows through the Forge-owned dispatcher +
 * ApplicationPort — never the RE CommandDispatcher.
 */
export const FORGE_ROUTED_COMMAND_TYPES: ReadonlySet<string> = new Set([
  FORGE_STORY_MARK_HOLD,
  FORGE_STORY_MARK_COMPLETE,
  FORGE_STORY_MARK_IN_PROGRESS,
  FORGE_RUN_APPEND_DETAIL,
  FORGE_RUN_SCOUT,
  FORGE_RUN_DIAGNOSIS,
  FORGE_RUN_ARCHITECT,
  FORGE_RUN_LEAD_PRE,
  FORGE_RUN_LEAD_IMPLEMENT,
  FORGE_RUN_SMITH,
  FORGE_RUN_SMITH_SPLIT,
  FORGE_RUN_LEAD_POST,
  FORGE_RUN_QA_REVIEW,
  FORGE_RUN_QA_VERIFY,
  FORGE_CLASSIFY_FAILURE,
  FORGE_RUN_SMITH_REPAIR,
  FORGE_RUN_ARCHITECT_REPAIR,
  FORGE_RUN_DEVOPS_REPAIR,
  FORGE_PUBLISH_CANDIDATE,
  FORGE_MIGRATE_DEV,
  FORGE_VERIFY_DEV_MIGRATION,
  FORGE_MIGRATE_PROD,
  FORGE_VERIFY_PROD_MIGRATION,
  FORGE_REFRESH_DERIVED_MODELS,
  FORGE_VERIFY_DERIVED_MODELS,
  FORGE_DEPLOY,
  FORGE_VERIFY_PRODUCTION,
])

/** Forge routability predicate: is this command in the Forge inventory? */
export function forgeCommandIsRouted(commandType: string): boolean {
  return FORGE_ROUTED_COMMAND_TYPES.has(commandType)
}
