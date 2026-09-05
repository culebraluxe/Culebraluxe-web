import type { ApplicationFacts, WorkflowSubject } from '../../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-FORGE-V9 Stage 3 — Forge command domain types (the "B" fork).
//
// The engine hosts two models with separate command domains:
//   A) Real estate -> lib/commands + workflow_app/command-types (deal.* etc.)
//   B) Forge        -> THIS Forge-owned domain (forge.* only)
//
// Forge commands are thin, deterministic state writes over an injected
// ForgeStateWriter. The core dispatcher + handlers depend ONLY on this
// interface (DB-free, unit-testable with fakes); the real Neon writer lives in
// db-state-writer.ts and is only wired when Forge actually executes on the
// engine. Nothing here imports RE's registry or dispatcher.
//
// Command set (grounded in forge-transition terminal actions + existing Forge
// DB writers):
//   forge.story.hold         -> mark story Human Hold
//   forge.story.complete     -> mark story Published Complete (outcome completed)
//   forge.story.in_progress  -> mark story In Progress (V6 lane-start)
//   forge.run.detail         -> append a durable run detail/audit line
//
// v1 FORGE_SDLC carries NO command-nodes, so none of these appear in XML yet;
// they are the executable surface a FORGE_SDLC-v2 command-node can reference.
// ---------------------------------------------------------------------------

/** Injected repository seam the Forge handlers write through (DI / fakes). */
export interface ForgeStateWriter {
  markStoryInProgress(storyId: string): Promise<void>
  /** Marks the story Human Hold. `reason` is a durable audit note when present. */
  markStoryHumanHold(storyId: string, reason: string): Promise<void>
  markStoryComplete(storyId: string): Promise<void>
  appendRunDetail(runId: string, detail: string): Promise<void>
}

/** Optional fact reader for the Forge ApplicationPort (subject -> facts). */
export type ForgeFactReader = (subject: WorkflowSubject) => Promise<ApplicationFacts>

export type ForgeCommandOutcome = 'success' | 'not_found' | 'validation_failure'

export type ForgeCommandEnvelope = {
  commandId?: string
  commandType: string
  input: Record<string, unknown>
}

export type ForgeCommandResult = {
  commandType: string
  outcome: ForgeCommandOutcome
  message?: string
}

export interface ForgeCommandHandler {
  handle(envelope: ForgeCommandEnvelope): Promise<ForgeCommandResult>
}

export interface ForgeCommandRegistry {
  register(commandType: string, handler: ForgeCommandHandler): void
  resolve(commandType: string): ForgeCommandHandler | undefined
  list(): string[]
}

/** Read a required string field from a command input; throws on malformed. */
export function requiredString(input: Record<string, unknown>, key: string): string {
  const value = input[key]
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`forge command input is missing required string field '${key}'`)
  }
  return value.trim()
}

/** Read an optional string field from a command input. */
export function optionalString(input: Record<string, unknown>, key: string): string {
  const value = input[key]
  return typeof value === 'string' ? value.trim() : ''
}
