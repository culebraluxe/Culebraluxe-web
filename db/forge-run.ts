import type { StoryboardStory } from './storyboard'
import type { RunMachineEvidence } from '../lib/forge-run-evidence'
import type { QueryExecutor, QueryRow } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/**
 * Runtime-facing Story shape. Stable Story identity/UI metadata remains on the
 * parent, while every execution-contract field comes from the immutable Run
 * snapshot created for this attempt.
 */
export type ForgeRunExecutionStory = StoryboardStory & {
  testMode?: string | null
  assayCommands?: string | null
  packetSha?: string | null
}

/**
 * Freeze the complete architect/execution contract onto a newly-created Run.
 * Existing historical runs are deliberately not backfilled from today's Story.
 */
export async function initializeForgeStoryRun(
  runId: string,
  input: { runType: string | null; agentRuntime: string | null },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story_run r
    set run_type = coalesce(${input.runType}, r.run_type),
        agent_runtime = coalesce(${input.agentRuntime}, r.agent_runtime),
        goal_snapshot = s.goal,
        scope_snapshot = s.scope,
        dependencies_snapshot = s.dependencies,
        preconditions_snapshot = s.preconditions,
        architect_brief_snapshot = s.architect_brief,
        context_refs_snapshot = s.context_refs,
        acceptance_criteria_snapshot = s.acceptance_criteria,
        postconditions_snapshot = s.postconditions,
        operating_surface_snapshot = s.operating_surface,
        test_mode_snapshot = s.test_mode,
        assay_commands_snapshot = s.assay_commands,
        packet_sha_snapshot = s.packet_sha,
        updated_at = now()
    from storyboard_story s
    where r.id = ${runId}
      and s.id = r.story_id
  `
}

/**
 * Build the execution-facing Story strictly from the frozen Run contract.
 * Parent Story values are retained only for stable identity/UI metadata that is
 * not part of the execution contract (id/title/workstream/priority/timestamps).
 * No execution lane may re-read mutable parent contract fields after this seam.
 */
export async function getForgeRunExecutionStory(
  runId: string,
  parent: StoryboardStory,
  execute?: QueryExecutor,
): Promise<ForgeRunExecutionStory | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select story_id, goal_snapshot, scope_snapshot, dependencies_snapshot,
      preconditions_snapshot, architect_brief_snapshot, context_refs_snapshot,
      acceptance_criteria_snapshot, postconditions_snapshot,
      operating_surface_snapshot, test_mode_snapshot, assay_commands_snapshot,
      packet_sha_snapshot
    from storyboard_story_run
    where id = ${runId}
  `
  const row = rows[0]
  if (!row) return null
  if (String(row.story_id) !== parent.id) {
    throw new Error(
      `Run ${runId} belongs to story ${String(row.story_id)}, not ${parent.id}`,
    )
  }

  return {
    ...parent,
    operatingSurface: (row.operating_surface_snapshot as StoryboardStory['operatingSurface']) ?? null,
    goal: (row.goal_snapshot as string | null) ?? null,
    scope: (row.scope_snapshot as string | null) ?? null,
    dependencies: (row.dependencies_snapshot as string | null) ?? null,
    preconditions: (row.preconditions_snapshot as string | null) ?? null,
    architectBrief: (row.architect_brief_snapshot as string | null) ?? null,
    contextRefs: (row.context_refs_snapshot as string | null) ?? null,
    acceptanceCriteria: (row.acceptance_criteria_snapshot as string | null) ?? null,
    postconditions: (row.postconditions_snapshot as string | null) ?? null,
    testMode: (row.test_mode_snapshot as string | null) ?? null,
    assayCommands: (row.assay_commands_snapshot as string | null) ?? null,
    packetSha: (row.packet_sha_snapshot as string | null) ?? null,
  }
}

/** Keep the Run's runtime identity durable even when it is resolved after start. */
export async function setForgeRunRuntime(
  runId: string,
  agentRuntime: string,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story_run
    set agent_runtime = ${agentRuntime}, updated_at = now()
    where id = ${runId}
  `
}

/** Persist generic machine facts. Null inputs preserve already-known facts. */
export async function recordForgeRunMachineEvidence(
  runId: string,
  evidence: RunMachineEvidence,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const detail = evidence.evidenceDetail?.trim() || null
  await q`
    update storyboard_story_run
    set base_commit_hash = coalesce(${evidence.baseCommitHash}, base_commit_hash),
        commands_total = coalesce(${evidence.commandsTotal}, commands_total),
        commands_passed = coalesce(${evidence.commandsPassed}, commands_passed),
        commands_failed = coalesce(${evidence.commandsFailed}, commands_failed),
        tests_total = coalesce(${evidence.testsTotal}, tests_total),
        tests_passed = coalesce(${evidence.testsPassed}, tests_passed),
        tests_failed = coalesce(${evidence.testsFailed}, tests_failed),
        policy_violation_count = coalesce(${evidence.policyViolationCount}, policy_violation_count),
        failure_code = coalesce(${evidence.failureCode}, failure_code),
        evidence_detail = case
          when ${detail}::text is null then evidence_detail
          when evidence_detail is null or evidence_detail = '' then ${detail}
          else evidence_detail || E'\n' || ${detail}
        end,
        updated_at = now()
    where id = ${runId}
  `
}

/** Append human/machine diagnostic detail without creating another history table. */
export async function appendForgeRunDetail(
  runId: string,
  detail: string,
  execute?: QueryExecutor,
): Promise<void> {
  const text = detail.trim()
  if (!text) return
  const q = execute ?? (await executor())
  await q`
    update storyboard_story_run
    set evidence_detail = case
          when evidence_detail is null or evidence_detail = ''
            then to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || ${text}
          else evidence_detail || E'\n' || to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || ${text}
        end,
        updated_at = now()
    where id = ${runId}
  `
}

type EvidenceRow = QueryRow & {
  base_commit_hash: string | null
  commands_total: number | null
  commands_passed: number | null
  commands_failed: number | null
  tests_total: number | null
  tests_passed: number | null
  tests_failed: number | null
  policy_violation_count: number | null
  failure_code: string | null
  evidence_detail: string | null
}

export async function getForgeRunMachineEvidence(
  runId: string,
  execute?: QueryExecutor,
): Promise<RunMachineEvidence | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select base_commit_hash, commands_total, commands_passed, commands_failed,
      tests_total, tests_passed, tests_failed, policy_violation_count,
      failure_code, evidence_detail
    from storyboard_story_run
    where id = ${runId}
  `
  const row = rows[0] as EvidenceRow | undefined
  if (!row) return null
  return {
    baseCommitHash: row.base_commit_hash ?? null,
    commandsTotal: row.commands_total ?? null,
    commandsPassed: row.commands_passed ?? null,
    commandsFailed: row.commands_failed ?? null,
    testsTotal: row.tests_total ?? null,
    testsPassed: row.tests_passed ?? null,
    testsFailed: row.tests_failed ?? null,
    policyViolationCount: row.policy_violation_count ?? null,
    failureCode: row.failure_code ?? null,
    evidenceDetail: row.evidence_detail ?? null,
  }
}
