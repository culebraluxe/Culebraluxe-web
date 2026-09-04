import { PortalWriteError } from '../lib/portal-write-error'
import { neonTx, type TxRunner } from './tx'
import type { QueryExecutor, QueryRow } from './query-executor'
import { mapStory, type StoryRow, type StoryboardStory } from './storyboard'
import type { AgentWorkItem } from './agent-work'
import type { LaneId } from '../agent-runtime/lanes'
import type { LeadRunPhase } from '../agent-runtime/lead-decision'

export type ForgeAgentWorkItem = AgentWorkItem & {
  lane: LaneId | null
  runPhase: LeadRunPhase | null
  playerId: string | null
  providerId: string | null
  modelId: string | null
  harnessId: string | null
  fieldId: string | null
  parallelGroupId: string | null
  parallelSlot: number | null
  parallelSize: number | null
  splitAssignment: string | null
  candidateShas: string[]
}

export type ForgeAgentWorkClaim = {
  workItem: ForgeAgentWorkItem
  story: StoryboardStory
}

type Row = QueryRow & {
  id: string
  story_id: string
  state: string
  priority: number
  queued_at: string
  claimed_at: string | null
  claimed_by: string | null
  started_at: string | null
  finished_at: string | null
  story_run_id: string | null
  error_text: string | null
  role: string | null
  lane: string | null
  run_phase: string | null
  model_profile: string | null
  player_id: string | null
  provider_id: string | null
  model_id: string | null
  harness_id: string | null
  field_id: string | null
  special_instructions: string | null
  runtime_adapter: string | null
  external_run_id: string | null
  attempts: number
  max_attempts: number
  execution_policy: string
  execution_environment: string | null
  parallel_group_id: string | null
  parallel_slot: number | null
  parallel_size: number | null
  split_assignment: string | null
  candidate_shas: string[] | null
  created_at: string
  updated_at: string
}

const ACTIVE_STATES = "('Claimed', 'Running', 'Paused')"

function mapRow(row: Row): ForgeAgentWorkItem {
  return {
    id: row.id,
    storyId: row.story_id,
    state: row.state as AgentWorkItem['state'],
    priority: row.priority,
    queuedAt: row.queued_at,
    claimedAt: row.claimed_at ?? null,
    claimedBy: row.claimed_by ?? null,
    startedAt: row.started_at ?? null,
    finishedAt: row.finished_at ?? null,
    storyRunId: row.story_run_id ?? null,
    errorText: row.error_text ?? null,
    role: row.role ?? null,
    modelProfile: row.model_profile ?? null,
    specialInstructions: row.special_instructions ?? null,
    runtimeAdapter: row.runtime_adapter ?? null,
    externalRunId: row.external_run_id ?? null,
    attempts: row.attempts ?? 0,
    maxAttempts: row.max_attempts ?? 3,
    executionPolicy: row.execution_policy ?? 'Unattended OK',
    executionEnvironment: row.execution_environment ?? null,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    lane: (row.lane as LaneId | null) ?? null,
    runPhase: (row.run_phase as LeadRunPhase | null) ?? null,
    playerId: row.player_id ?? null,
    providerId: row.provider_id ?? null,
    modelId: row.model_id ?? null,
    harnessId: row.harness_id ?? null,
    fieldId: row.field_id ?? null,
    parallelGroupId: row.parallel_group_id ?? null,
    parallelSlot: row.parallel_slot ?? null,
    parallelSize: row.parallel_size ?? null,
    splitAssignment: row.split_assignment ?? null,
    candidateShas: row.candidate_shas ?? [],
  }
}

const columns = `id, story_id, state, priority, queued_at, claimed_at, claimed_by,
  started_at, finished_at, story_run_id, error_text, role, lane, run_phase,
  model_profile, player_id, provider_id, model_id, harness_id, field_id,
  special_instructions, runtime_adapter, external_run_id, attempts, max_attempts,
  execution_policy, execution_environment, parallel_group_id, parallel_slot,
  parallel_size, split_assignment, candidate_shas, created_at, updated_at`

// Neon tagged templates cannot interpolate SQL identifiers/column lists. Keep
// the actual lists literal in each query below; `columns` documents parity.
void columns

export async function getForgeAgentWorkItem(
  workItemId: string,
  q: QueryExecutor,
): Promise<ForgeAgentWorkItem | null> {
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, lane, run_phase,
      model_profile, player_id, provider_id, model_id, harness_id, field_id,
      special_instructions, runtime_adapter, external_run_id, attempts, max_attempts,
      execution_policy, execution_environment, parallel_group_id, parallel_slot,
      parallel_size, split_assignment, candidate_shas, created_at, updated_at
    from agent_work_item where id = ${workItemId}
  `
  return rows[0] ? mapRow(rows[0] as Row) : null
}

export async function listForgeAgentWorkItems(q: QueryExecutor): Promise<ForgeAgentWorkItem[]> {
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, lane, run_phase,
      model_profile, player_id, provider_id, model_id, harness_id, field_id,
      special_instructions, runtime_adapter, external_run_id, attempts, max_attempts,
      execution_policy, execution_environment, parallel_group_id, parallel_slot,
      parallel_size, split_assignment, candidate_shas, created_at, updated_at
    from agent_work_item order by queued_at desc, id
  `
  return rows.map((row) => mapRow(row as Row))
}

export async function listForgeAgentWorkForStory(
  storyId: string,
  q: QueryExecutor,
): Promise<ForgeAgentWorkItem[]> {
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, lane, run_phase,
      model_profile, player_id, provider_id, model_id, harness_id, field_id,
      special_instructions, runtime_adapter, external_run_id, attempts, max_attempts,
      execution_policy, execution_environment, parallel_group_id, parallel_slot,
      parallel_size, split_assignment, candidate_shas, created_at, updated_at
    from agent_work_item where story_id = ${storyId}
    order by queued_at asc, id
  `
  return rows.map((row) => mapRow(row as Row))
}

export type EnqueueForgeAgentWorkInput = {
  storyId: string
  role: string
  lane: LaneId
  runPhase?: LeadRunPhase | null
  modelProfile: string
  playerId: string
  providerId: string
  modelId: string
  harnessId: string
  fieldId: string
  specialInstructions?: string | null
  candidateShas?: string[]
  priority?: number
  maxAttempts?: number
  executionPolicy?: string
  executionEnvironment?: string | null
  parallelGroupId?: string | null
  parallelSlot?: number | null
  parallelSize?: number | null
  splitAssignment?: string | null
}

export async function enqueueForgeAgentWorkCommand(
  input: EnqueueForgeAgentWorkInput,
  q: QueryExecutor,
): Promise<ForgeAgentWorkItem> {
  const candidateShas = (input.candidateShas ?? []).map((sha) => sha.trim()).filter(Boolean)
  const parallel = Boolean(input.parallelGroupId)
  const rows = parallel
    ? await q`
        insert into agent_work_item (
          story_id, state, priority, role, lane, run_phase, model_profile,
          player_id, provider_id, model_id, harness_id, field_id,
          special_instructions, candidate_shas, max_attempts, execution_policy,
          execution_environment, parallel_group_id, parallel_slot, parallel_size,
          split_assignment
        ) values (
          ${input.storyId}, 'Ready', ${input.priority ?? 0}, ${input.role}, ${input.lane},
          ${input.runPhase ?? null}, ${input.modelProfile}, ${input.playerId},
          ${input.providerId}, ${input.modelId}, ${input.harnessId}, ${input.fieldId},
          ${input.specialInstructions ?? null}, ${candidateShas}, ${input.maxAttempts ?? 3},
          ${input.executionPolicy ?? 'Unattended OK'}, ${input.executionEnvironment ?? 'DEV'},
          ${input.parallelGroupId ?? null}, ${input.parallelSlot ?? null},
          ${input.parallelSize ?? null}, ${input.splitAssignment ?? null}
        )
        returning id, story_id, state, priority, queued_at, claimed_at, claimed_by,
          started_at, finished_at, story_run_id, error_text, role, lane, run_phase,
          model_profile, player_id, provider_id, model_id, harness_id, field_id,
          special_instructions, runtime_adapter, external_run_id, attempts, max_attempts,
          execution_policy, execution_environment, parallel_group_id, parallel_slot,
          parallel_size, split_assignment, candidate_shas, created_at, updated_at
      `
    : await q`
        insert into agent_work_item (
          story_id, state, priority, role, lane, run_phase, model_profile,
          player_id, provider_id, model_id, harness_id, field_id,
          special_instructions, candidate_shas, max_attempts, execution_policy,
          execution_environment
        ) values (
          ${input.storyId}, 'Ready', ${input.priority ?? 0}, ${input.role}, ${input.lane},
          ${input.runPhase ?? null}, ${input.modelProfile}, ${input.playerId},
          ${input.providerId}, ${input.modelId}, ${input.harnessId}, ${input.fieldId},
          ${input.specialInstructions ?? null}, ${candidateShas}, ${input.maxAttempts ?? 3},
          ${input.executionPolicy ?? 'Unattended OK'}, ${input.executionEnvironment ?? 'DEV'}
        )
        on conflict (story_id) where state in ('Ready', 'Claimed', 'Running', 'Paused')
          and parallel_group_id is null
        do update set
          role = excluded.role,
          lane = excluded.lane,
          run_phase = excluded.run_phase,
          model_profile = excluded.model_profile,
          player_id = excluded.player_id,
          provider_id = excluded.provider_id,
          model_id = excluded.model_id,
          harness_id = excluded.harness_id,
          field_id = excluded.field_id,
          special_instructions = excluded.special_instructions,
          candidate_shas = excluded.candidate_shas,
          max_attempts = excluded.max_attempts,
          execution_policy = excluded.execution_policy,
          execution_environment = excluded.execution_environment,
          updated_at = now()
        returning id, story_id, state, priority, queued_at, claimed_at, claimed_by,
          started_at, finished_at, story_run_id, error_text, role, lane, run_phase,
          model_profile, player_id, provider_id, model_id, harness_id, field_id,
          special_instructions, runtime_adapter, external_run_id, attempts, max_attempts,
          execution_policy, execution_environment, parallel_group_id, parallel_slot,
          parallel_size, split_assignment, candidate_shas, created_at, updated_at
      `
  const row = rows[0] as Row | undefined
  if (!row) throw new PortalWriteError('conflict', `Unable to enqueue work for story "${input.storyId}".`)
  return mapRow(row)
}

/**
 * Concurrency is intentionally NOT a general worker pool. A second/third claim
 * is allowed only when the currently active work belongs to one explicit Lead
 * Smith split group. All serial work remains system-wide serialized.
 */
export function forgeParallelWorkerLimit(
  env: Readonly<Record<string, string | undefined>> = process.env,
): number {
  const value = Number(env.FORGE_PARALLEL_SMITH_WORKERS ?? 3)
  if (!Number.isInteger(value) || value < 1) return 1
  return Math.min(3, value)
}

async function claimAllowed(
  tx: QueryExecutor,
  target: { id: string; parallel_group_id: string | null },
): Promise<boolean> {
  const active = await tx`
    select id, parallel_group_id, lane
    from agent_work_item
    where state in ('Claimed', 'Running', 'Paused')
    order by claimed_at nulls last, id
  `
  if (active.length === 0) return true
  if (!target.parallel_group_id) return false
  const limit = forgeParallelWorkerLimit()
  if (active.length >= limit) return false
  return active.every(
    (row) => row.lane === 'smith' && row.parallel_group_id === target.parallel_group_id,
  )
}

async function claimTarget(
  tx: QueryExecutor,
  workItemId: string,
  workerId: string,
): Promise<ForgeAgentWorkItem | null> {
  const targets = await tx`
    select id, parallel_group_id from agent_work_item
    where id = ${workItemId} and state = 'Ready'
    for update
  `
  const target = targets[0] as { id: string; parallel_group_id: string | null } | undefined
  if (!target || !(await claimAllowed(tx, target))) return null
  const rows = await tx`
    update agent_work_item
    set state = 'Claimed', claimed_at = now(), claimed_by = ${workerId},
        attempts = attempts + 1, updated_at = now()
    where id = ${workItemId} and state = 'Ready'
    returning id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, lane, run_phase,
      model_profile, player_id, provider_id, model_id, harness_id, field_id,
      special_instructions, runtime_adapter, external_run_id, attempts, max_attempts,
      execution_policy, execution_environment, parallel_group_id, parallel_slot,
      parallel_size, split_assignment, candidate_shas, created_at, updated_at
  `
  return rows[0] ? mapRow(rows[0] as Row) : null
}

export async function claimSpecificForgeAgentWork(
  workItemId: string,
  workerId: string,
  runner: TxRunner = neonTx,
): Promise<ForgeAgentWorkItem | null> {
  return runner(async (tx) => {
    await tx`select pg_advisory_xact_lock(cast(9000212 as bigint))`
    return claimTarget(tx, workItemId, workerId)
  })
}

export async function claimNextForgeAgentWork(
  workerId: string,
  runner: TxRunner = neonTx,
): Promise<ForgeAgentWorkClaim | null> {
  return runner(async (tx) => {
    await tx`select pg_advisory_xact_lock(cast(9000212 as bigint))`
    const active = await tx`
      select id, parallel_group_id, lane
      from agent_work_item
      where state in ('Claimed', 'Running', 'Paused')
      order by claimed_at nulls last, id
    `

    let candidates
    if (active.length === 0) {
      candidates = await tx`
        select id, parallel_group_id
        from agent_work_item
        where state = 'Ready'
        order by priority desc, queued_at asc, id
        limit 1
      `
    } else {
      const group = active[0]?.parallel_group_id as string | null | undefined
      const sameSplit = Boolean(group) && active.every(
        (row) => row.lane === 'smith' && row.parallel_group_id === group,
      )
      if (!sameSplit || active.length >= forgeParallelWorkerLimit()) return null
      candidates = await tx`
        select id, parallel_group_id
        from agent_work_item
        where state = 'Ready'
          and lane = 'smith'
          and parallel_group_id = ${group ?? null}
        order by parallel_slot asc, id
        limit 1
      `
    }

    const candidate = candidates[0] as { id: string; parallel_group_id: string | null } | undefined
    if (!candidate) return null
    const workItem = await claimTarget(tx, candidate.id, workerId)
    if (!workItem) return null
    const storyRows = await tx`
      select id, workstream, operating_surface, title, priority, status, notes,
        batch, goal, scope, dependencies, preconditions, architect_brief, context_refs,
        acceptance_criteria, postconditions, architect_brief_updated_at,
        completion, rollup, planned_start_at, actual_start_at, completed_at,
        created_at, updated_at
      from storyboard_story where id = ${workItem.storyId}
    `
    const storyRow = storyRows[0] as StoryRow | undefined
    if (!storyRow) throw new PortalWriteError('not-found', `Story "${workItem.storyId}" was not found.`)
    return { workItem, story: mapStory(storyRow) }
  })
}

export function validateForgeWorkRouting(item: ForgeAgentWorkItem): string | null {
  if (!item.lane) return 'missing required lane'
  if (item.lane === 'lead' && !item.runPhase) return 'Lead work requires run_phase'
  if (!item.playerId) return 'missing required player_id'
  if (!item.providerId) return 'missing required provider_id'
  if (!item.modelId) return 'missing required model_id'
  if (!item.harnessId) return 'missing required harness_id'
  if (!item.fieldId) return 'missing required field_id'
  if (item.parallelGroupId) {
    if (item.lane !== 'smith') return 'only Smith may belong to a parallel_group_id'
    if (!item.parallelSlot || !item.parallelSize || !item.splitAssignment) {
      return 'parallel Smith work requires slot, size, and split_assignment'
    }
  }
  return null
}
