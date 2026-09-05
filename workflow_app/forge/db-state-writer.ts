import type { ForgeStateWriter } from './forge-state-writer'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 — Real Neon-backed ForgeStateWriter.
//
// Thin adapter over the existing Forge DB writers. Imported lazily (dynamic
// import below) so the pure command domain never pulls the database layer; this
// module is only loaded when Forge actually wires a writer (engine execution,
// operator tooling). The DB modules themselves resolve their executor lazily,
// so importing this never opens a connection.
// ---------------------------------------------------------------------------

/**
 * The Neon-backed writer. Each call is the canonical domain mutation that a
 * forge.* command's handler performs.
 */
export async function createDbForgeStateWriter(): Promise<ForgeStateWriter> {
  const [storyState, forgeRun] = await Promise.all([
    import('../../db/forge-story-state'),
    import('../../db/forge-run'),
  ])
  return {
    async markStoryInProgress(storyId) {
      await storyState.markForgeStoryInProgress(storyId)
    },
    async markStoryHumanHold(storyId, reason) {
      await storyState.markForgeStoryHumanHold(storyId, reason)
    },
    async markStoryComplete(storyId) {
      await storyState.markForgeStoryPublishedComplete(storyId)
    },
    async appendRunDetail(runId, detail) {
      await forgeRun.appendForgeRunDetail(runId, detail)
    },
  }
}
