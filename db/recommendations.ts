// ---------------------------------------------------------------------------
// CORE-DAILY-07/08 — relationship recommendation read projection + suppression.
// Deterministic, explainable, evidence-backed. Never canonical obligations;
// regeneration never creates duplicate obligations. Dismissals are idempotent.
// ---------------------------------------------------------------------------
import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import {
  evaluateRecommendations,
  type Recommendation,
  type RecommendationCode,
} from '../lib/relationship-intel/recommendations'
import { summarizeRelationshipEvidence } from '../lib/relationship-intel/relationship-context'
import { getRelationshipEvidenceForPersons } from './relationship-evidence'

const DUE_SOON_WINDOW_MS = 3 * 24 * 60 * 60 * 1000
const QUIET_AFTER_MS = 45 * 24 * 60 * 60 * 1000

type FollowUpRow = {
  id: string
  person_id: string
  due_at: string | null
  title: string | null
}

/**
 * Deterministic recommendations for persons with open follow-ups and/or
 * meaningful relationship evidence. A person is evaluated only when they have
 * an actionable follow-up or relationship evidence (bounded read).
 */
export async function getRecommendations(
  execute: QueryExecutor = sql,
): Promise<Recommendation[]> {
  const nowIso = new Date().toISOString()

  const followUpRows = (await execute`
    select id, person_id, due_at, title
    from task
    where status = 'open'
      and person_id is not null
    order by person_id, due_at asc nulls last
  `) as FollowUpRow[]

  const byPerson = new Map<string, FollowUpRow[]>()
  for (const r of followUpRows) {
    ;(byPerson.get(r.person_id) ?? byPerson.set(r.person_id, []).get(r.person_id)!).push(r)
  }

  const personIds = Array.from(new Set([...byPerson.keys()]))
  const evidenceByPerson = await getRelationshipEvidenceForPersons(personIds, execute)
  const dismissalRows = (await execute`
    select person_id, code from relationship_recommendation_dismissal
    where person_id = any (${personIds})
  `) as { person_id: string; code: string }[]
  const dismissed = new Set<string>()
  for (const d of dismissalRows) dismissed.add(`${d.person_id}:${d.code}`)

  const out: Recommendation[] = []
  for (const personId of personIds) {
    const followUps = (byPerson.get(personId) ?? []).map((f) => ({
      id: f.id,
      dueAt: f.due_at ? new Date(f.due_at).toISOString() : null,
      title: f.title,
    }))
    const summary = summarizeRelationshipEvidence(evidenceByPerson[personId] ?? [])
    const recs = evaluateRecommendations({
      personId,
      followUps,
      evidence: summary.hasEvidence
        ? {
            lastMeaningfulContactAt: summary.lastMeaningfulContactAt,
            lastInboundAt: summary.lastInboundAt,
            lastOutboundAt: summary.lastOutboundAt,
            twoWay: summary.twoWay,
            hasEvidence: true,
            coverageLimited: summary.coverageLimited,
          }
        : undefined,
      dismissed,
      nowIso,
      dueSoonWindowMs: DUE_SOON_WINDOW_MS,
      quietAfterMs: QUIET_AFTER_MS,
    })
    out.push(...recs)
  }
  return out
}

/** Idempotent dismissal — a dismissed key stays suppressed across regeneration. */
export async function dismissRecommendation(
  personId: string,
  code: RecommendationCode,
  actorUserId?: string | null,
  execute: QueryExecutor = sql,
): Promise<void> {
  await execute`
    insert into relationship_recommendation_dismissal (person_id, code, dismissed_by)
    values (${personId}, ${code}, ${actorUserId ?? null})
    on conflict (person_id, code) do nothing
  `
}
