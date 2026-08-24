import { buildIssuedExecutionSlots, type IssuedExecutionSlot } from './execution'

// ---------------------------------------------------------------------------
// CRM-27 (Phase 1 second pass) — Canonical issued-participant boundary.
//
// One canonicalization boundary BEFORE buildIssuedExecutionSlots. It resolves the
// mutable participant sources into a deterministic, immutable issued-slot set and
// provides a strict parser for the snapshot persisted at issuance.
//
// DEDUP RULE (authoritative):
//   Within the SAME execution role, collapse rows that share a STRONG identity —
//   1. role + personId; otherwise
//   2. role + normalized email.
//   Different roles are NEVER collapsed even for the same person. Rows with no
//   strong identity anchor (no personId AND no email) are preserved conservatively
//   (never merged — two possibly distinct people stay separate).
//
// ORDERING RULE (deterministic): the same resolved participant set always yields
// the same slot identities regardless of source order. Rows are sorted by
// (role, strong-identity-key, name) before dedup + ROLE:sequence assignment, so
// overlapping sources (seeded document_form_participant + source deal_participant)
// cannot reorder or duplicate slots.
// ---------------------------------------------------------------------------

export type ExecutionParticipantInput = {
  role: string
  personId: string | null
  name: string
  email: string | null
}

/** Normalized email key (trim + lowercase); null when absent. */
export function normalizeEmail(value: string | null | undefined): string | null {
  if (!value) return null
  const trimmed = value.trim().toLowerCase()
  return trimmed === '' ? null : trimmed
}

/** Canonicalize resolved participants into deterministic immutable issued slots. */
export function canonicalizeExecutionParticipants(
  people: readonly ExecutionParticipantInput[],
): IssuedExecutionSlot[] {
  const strongKey = (p: ExecutionParticipantInput): string | null => {
    if (p.personId) return `person:${p.personId}`
    const email = normalizeEmail(p.email)
    if (email) return `email:${email}`
    return null
  }

  // Deterministic ordering: (role, strong-identity-key, name, email). Weak rows
  // (no strong key) sort by a `weak:` key derived from name — conservatively kept,
  // never merged.
  const sorted = [...people].sort((a, b) => {
    if (a.role !== b.role) return a.role < b.role ? -1 : 1
    const aKey = strongKey(a) ?? `weak:${(a.name || '').toLowerCase()}`
    const bKey = strongKey(b) ?? `weak:${(b.name || '').toLowerCase()}`
    if (aKey !== bKey) return aKey < bKey ? -1 : 1
    return (normalizeEmail(a.email) ?? '').localeCompare(normalizeEmail(b.email) ?? '')
  })

  const seen = new Set<string>()
  const deduped: ExecutionParticipantInput[] = []
  for (const p of sorted) {
    const key = strongKey(p)
    if (key) {
      const dedupKey = `${p.role}::${key}`
      if (seen.has(dedupKey)) continue
      seen.add(dedupKey)
    }
    deduped.push(p)
  }
  return buildIssuedExecutionSlots(deduped)
}

export type ParsedParticipants =
  | { ok: true; slots: IssuedExecutionSlot[] }
  | { ok: false; error: string }

/**
 * STRICT parser for `source_snapshot.issuedParticipants`. Fails closed on:
 *   - missing / empty snapshot;
 *   - malformed entries;
 *   - duplicate slot ids;
 *   - a role inconsistent with its slot id;
 *   - a required slot with no usable participant identity anchor.
 * An eligible legacy/malformed document must return a truthful failure — never
 * fabricated role-only automatic evidence.
 */
export function parseIssuedParticipants(raw: unknown): ParsedParticipants {
  if (!Array.isArray(raw) || raw.length === 0) {
    return { ok: false, error: 'issuedParticipants is missing or empty.' }
  }
  const seen = new Set<string>()
  const slots: IssuedExecutionSlot[] = []
  for (let i = 0; i < raw.length; i++) {
    const entry = raw[i]
    if (typeof entry !== 'object' || entry === null) {
      return { ok: false, error: `issuedParticipants[${i}] is not an object.` }
    }
    const e = entry as Record<string, unknown>
    const slotId = typeof e.slotId === 'string' ? e.slotId : ''
    const role = typeof e.role === 'string' ? e.role : ''
    const name = typeof e.name === 'string' ? e.name : ''
    const personId = typeof e.personId === 'string' && e.personId !== '' ? e.personId : null
    const email = typeof e.email === 'string' && e.email !== '' ? e.email : null
    const required = e.required !== false
    const order = typeof e.order === 'number' ? e.order : i

    if (!slotId) return { ok: false, error: `issuedParticipants[${i}] is missing slotId.` }
    if (!role) return { ok: false, error: `issuedParticipants[${i}] is missing role.` }
    if (seen.has(slotId)) return { ok: false, error: `duplicate slotId '${slotId}'.` }
    seen.add(slotId)
    if (!slotId.startsWith(`${role}:`)) {
      return { ok: false, error: `role '${role}' is inconsistent with slotId '${slotId}'.` }
    }
    if (required && !personId && !email) {
      return {
        ok: false,
        error: `required slot '${slotId}' has no usable participant identity anchor.`,
      }
    }
    slots.push({ slotId, role, personId, name, email, required, order })
  }
  return { ok: true, slots }
}

/**
 * Resolve a participant selection to EXACTLY ONE issued slot. Rejects when no
 * slot matches or the selection is ambiguous (multiple matches). Selection is by
 * (role, personId); falls back to (role, normalized email) when personId is absent.
 */
export function resolveIssuedSlot(
  slots: readonly IssuedExecutionSlot[],
  selection: { role?: string | null; personId?: string | null; email?: string | null },
): { ok: true; slot: IssuedExecutionSlot } | { ok: false; error: string } {
  let candidates = slots
  if (selection.role) candidates = candidates.filter((s) => s.role === selection.role)
  if (selection.personId) {
    candidates = candidates.filter((s) => s.personId === selection.personId)
  } else if (selection.email) {
    const ne = normalizeEmail(selection.email)
    candidates = candidates.filter((s) => normalizeEmail(s.email) === ne)
  }
  if (candidates.length === 0) {
    return { ok: false, error: 'No issued execution slot matches the selected participant.' }
  }
  if (candidates.length > 1) {
    return {
      ok: false,
      error: 'Multiple issued slots match the selected participant; the selection is ambiguous.',
    }
  }
  return { ok: true, slot: candidates[0] }
}
