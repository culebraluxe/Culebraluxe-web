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

// Stable Forge command identifiers (the executable surface of the Forge model).
export const FORGE_STORY_MARK_HOLD = 'forge.story.hold'
export const FORGE_STORY_MARK_COMPLETE = 'forge.story.complete'
export const FORGE_STORY_MARK_IN_PROGRESS = 'forge.story.in_progress'
export const FORGE_RUN_APPEND_DETAIL = 'forge.run.detail'

/** Command-node types referenced by FORGE_SDLC XML (none yet in v1). */
export const FORGE_XML_COMMAND_NODE_TYPES: ReadonlySet<string> = new Set([])

/**
 * Every Forge command with a registered handler in the Forge command domain
 * (workflow_app/forge/forge-command.ts). Routing flows through the Forge-owned
 * dispatcher + ApplicationPort — never the RE CommandDispatcher.
 */
export const FORGE_ROUTED_COMMAND_TYPES: ReadonlySet<string> = new Set([
  FORGE_STORY_MARK_HOLD,
  FORGE_STORY_MARK_COMPLETE,
  FORGE_STORY_MARK_IN_PROGRESS,
  FORGE_RUN_APPEND_DETAIL,
])

/** Forge routability predicate: is this command in the Forge inventory? */
export function forgeCommandIsRouted(commandType: string): boolean {
  return FORGE_ROUTED_COMMAND_TYPES.has(commandType)
}
