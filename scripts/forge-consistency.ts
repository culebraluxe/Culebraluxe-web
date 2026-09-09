// ---------------------------------------------------------------------------
// ENG-FORGE-V5-08 — Forge Consistency Janitor (read-only audit CLI).
// Usage: APP_ENV=<dev|production> node --env-file=.env.local --import tsx \
//        scripts/forge-consistency.ts [--story <id>] [--json]
// Detects durable-state invariant violations (see workflow_app/forge/forge-consistency)
// and REPORTS them. This is audit-only — it never mutates evidence.
// ---------------------------------------------------------------------------
import { readForgeWorkflowEvidence } from '../db/forge-workflow-evidence'
import { listActiveForgeRoleTasks } from '../workflow_app/forge/forge-engine-runtime'
import { listStoryboardStories, listStoryExecutionSummaries } from '../db/storyboard'
import {
  type StoryConsistencySnapshot,
  type ConsistencyViolation,
  auditStoryConsistency,
} from '../workflow_app/forge/forge-consistency'

const args = process.argv.slice(2)
const onlyStory = args.includes('--story') ? args[args.indexOf('--story') + 1] : null
const asJson = args.includes('--json')

async function main() {
  const stories = await listStoryboardStories()
  if (!stories) throw new Error('storyboard unavailable')
  // V1 (pre-V2) legacy-closed stories are exempt from Forge consistency audits.
  const v1LegacyById = new Map<string, boolean>()
  try {
    const { sql } = await import('../db/client')
    const rows = (await sql`select id, forge_v1_legacy from storyboard_story`) as Array<{
      id: string
      forge_v1_legacy: boolean
    }>
    for (const r of rows) v1LegacyById.set(r.id, r.forge_v1_legacy === true)
  } catch {
    /* table/column unavailable in this env -> no exemptions (audits everything) */
  }
  const runByStory = new Map<string, string | null>()
  try {
    const summaries = await listStoryExecutionSummaries()
    for (const s of summaries ?? []) runByStory.set(s.storyId, s.latestRunResult ?? null)
  } catch {
    /* run projection unavailable -> I5 stays silent; everything else still audits */
  }

  const targets = onlyStory ? stories.filter((s) => s.id === onlyStory) : stories
  if (onlyStory && targets.length === 0) throw new Error(`story ${onlyStory} not found`)

  const violations: ConsistencyViolation[] = []
  const audited: string[] = []
  for (const story of targets) {
    // Engine role-task lookups need the engine DB; when it is not configured the
    // janitor still audits the app-durable evidence (openTaskCount treated as 0).
    let openTaskCount = 0
    try {
      openTaskCount = (await listActiveForgeRoleTasks(story.id)).length
    } catch {
      openTaskCount = 0
    }
    let evidence = null
    try {
      const e = await readForgeWorkflowEvidence(story.id)
      evidence = {
        candidateSha: e.candidateSha ?? null,
        qaPassed: e.qaPassed ?? null,
        qaVerifiedSha: e.qaVerifiedSha ?? null,
        publishedSha: e.publishedSha ?? null,
        deployedSha: e.deployedSha ?? null,
      }
    } catch {
      evidence = null
    }
    const snapshot: StoryConsistencySnapshot = {
      storyId: story.id,
      storyStatus: story.status ?? '',
      v1Legacy: v1LegacyById.get(story.id) ?? false,
      evidence,
      run: { resultStatus: runByStory.get(story.id) ?? null },
      engineNodesCompleted: [],
      openTaskCount,
    }
    audited.push(story.id)
    violations.push(...auditStoryConsistency(snapshot))
  }

  if (asJson) {
    console.log(JSON.stringify({ audited: audited.length, violations }, null, 2))
    return
  }
  console.log(`audited=${audited.length} violations=${violations.length}`)
  for (const v of violations) {
    console.log(`[${v.severity}] ${v.storyId} ${v.kind} :: ${v.detail}`)
  }
}

main().catch((e) => {
  console.error(String((e as Error)?.stack ?? e))
  process.exitCode = 1
})
