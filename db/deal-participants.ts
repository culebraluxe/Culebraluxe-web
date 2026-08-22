import { PortalWriteError } from '../lib/portal-write-error'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// Canonical deal_participant repository (CRM-13).
//
// deal_participant is THE canonical participant model. A small checked
// structural role (client/owner/seller/other) plus an optional role_label
// (<= 120 chars) that carries the SME long tail (lender, inspector,
// appraiser, notario, title, ...). New SME roles are application-curated
// labels, never schema migrations. There is deliberately no second
// participant abstraction.
//
// Invariants (DB layer: migration 034; mirrored here as service-level checks
// so writes fail with a clean PortalWriteError instead of a raw unique
// violation):
//   - at most one ACTIVE structural participant (client/owner/seller) per deal
//   - at most one ACTIVE role='other' participant per role_label per deal
//     (case-insensitive)
// The single-subject rule (person XOR user) is enforced by the schema check
// deal_participant_single_subject.
//
// The default executor is resolved lazily (mirroring db/storyboard.ts) so
// importing this module never requires a DATABASE_URL; tests inject an
// in-memory fake.
// ---------------------------------------------------------------------------

export const PARTICIPANT_STRUCTURAL_ROLES = ['client', 'owner', 'seller'] as const

export type ParticipantStructuralRole =
  (typeof PARTICIPANT_STRUCTURAL_ROLES)[number]

export const PARTICIPANT_ROLE_LABEL_MAX = 120

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/**
 * Invariant: no active structural participant (client/owner/seller) with the
 * given role may exist for the deal. Throws PortalWriteError('conflict')
 * otherwise.
 */
export async function assertNoActiveStructuralRole(
  dealId: string,
  role: ParticipantStructuralRole,
  execute?: QueryExecutor,
): Promise<void> {
  const run = execute ?? (await executor())
  const rows = await run`
    select id
    from deal_participant
    where deal_id = ${dealId}
      and role = ${role}
      and active = true
    limit 1
  `
  if (rows.length > 0) {
    throw new PortalWriteError(
      'conflict',
      `An active ${role} participant already exists for this deal.`,
    )
  }
}

/**
 * Invariant: no other active role='other' participant with the same
 * role_label (case-insensitive) may exist for the deal. `excludeParticipantId`
 * lets an update ignore the row being updated itself. Throws
 * PortalWriteError('conflict') otherwise.
 */
export async function assertNoDuplicateRoleLabel(
  dealId: string,
  roleLabel: string,
  excludeParticipantId: string | null,
  execute?: QueryExecutor,
): Promise<void> {
  const run = execute ?? (await executor())
  // `id is distinct from $1` doubles as the exclusion predicate and gives the
  // parameter its uuid type from `id` (a bare `$n is null` would be
  // untypable — Postgres error 42P18). When excludeParticipantId is null the
  // predicate is `id is distinct from null` — true for every row, so no
  // exclusion happens.
  const rows = await run`
    select id
    from deal_participant
    where deal_id = ${dealId}
      and role = 'other'
      and active = true
      and lower(role_label) = lower(${roleLabel})
      and id is distinct from ${excludeParticipantId ?? null}
    limit 1
  `
  if (rows.length > 0) {
    throw new PortalWriteError(
      'conflict',
      `An active "${roleLabel}" participant already exists for this deal.`,
    )
  }
}

export async function addOtherParticipant(
  input: {
    dealId: string
    personId?: string
    userId?: string
    roleLabel: string
  },
  execute?: QueryExecutor,
): Promise<{ participantId: string }> {
  const run = execute ?? (await executor())

  if (!input.personId && !input.userId) {
    throw new PortalWriteError(
      'validation',
      'Exactly one of person or user is required.',
    )
  }
  if (input.personId && input.userId) {
    throw new PortalWriteError(
      'validation',
      'Exactly one of person or user is required.',
    )
  }
  const roleLabel = input.roleLabel.trim()
  if (!roleLabel) {
    throw new PortalWriteError(
      'validation',
      'A role label is required for other participants.',
    )
  }
  if (roleLabel.length > PARTICIPANT_ROLE_LABEL_MAX) {
    throw new PortalWriteError('validation', 'Role label is too long.')
  }

  // Long-tail dedupe: one active role_label per deal (case-insensitive).
  await assertNoDuplicateRoleLabel(input.dealId, roleLabel, null, run)

  const rows = await run`
    insert into deal_participant (deal_id, person_id, user_id, role, role_label, active)
    values (
      ${input.dealId}, ${input.personId ?? null}, ${input.userId ?? null},
      'other', ${roleLabel}, true
    )
    returning id
  `
  const id = (rows[0] as { id?: string } | undefined)?.id
  if (!id) throw new Error('Participant could not be created.')
  return { participantId: id }
}

export async function endParticipant(
  participantId: string,
  execute?: QueryExecutor,
): Promise<{ participantId: string }> {
  const run = execute ?? (await executor())
  const rows = await run`
    update deal_participant
    set active = false, ended_at = now(), updated_at = now()
    where id = ${participantId}
      and active = true
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'conflict',
      'Participant not found or already ended.',
    )
  }
  return { participantId }
}

export async function updateParticipantRoleLabel(
  participantId: string,
  roleLabel: string,
  execute?: QueryExecutor,
): Promise<{ participantId: string }> {
  const run = execute ?? (await executor())

  const normalized = roleLabel.trim()
  if (!normalized) {
    throw new PortalWriteError('validation', 'Role label is required.')
  }
  if (normalized.length > PARTICIPANT_ROLE_LABEL_MAX) {
    throw new PortalWriteError('validation', 'Role label is too long.')
  }

  // Resolve the participant's deal so the dedupe check is scoped to it.
  const found = (await run`
    select deal_id, active
    from deal_participant
    where id = ${participantId}
      and role = 'other'
    limit 1
  `) as QueryRow[]
  const target = found[0] as { deal_id?: string; active?: boolean } | undefined
  if (!target?.deal_id) {
    throw new PortalWriteError(
      'conflict',
      'Participant not found or not role=other.',
    )
  }

  // Long-tail dedupe against the other active participants of the same deal,
  // excluding the row being relabeled.
  if (target.active === true) {
    await assertNoDuplicateRoleLabel(target.deal_id, normalized, participantId, run)
  }

  const rows = await run`
    update deal_participant
    set role_label = ${normalized}, updated_at = now()
    where id = ${participantId}
      and role = 'other'
    returning id
  `
  if (rows.length === 0) {
    throw new PortalWriteError(
      'conflict',
      'Participant not found or not role=other.',
    )
  }
  return { participantId }
}
