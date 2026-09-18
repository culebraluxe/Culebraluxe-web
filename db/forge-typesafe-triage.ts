import type { QueryExecutor, QueryRow } from './query-executor'
import { recordToolArtifact } from './forge-artifact'
import { isForgeFailureClass, type ForgeFailureClass } from '../workflow_app/forge/failure-classifier'
import {
  REVIEW_KIND, TRIAGE_KIND, TRIAGE_MODEL, TRIAGE_PROMPT_VERSION, TriageInputError,
  isFailureSource, triageText, type FailureSource, type TriageObservation,
} from '../workflow_app/forge/typesafe-failure-triage'

function detailObject(value: unknown): Record<string, unknown> {
  if (typeof value === 'string') return detailObject(JSON.parse(value))
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {}
}

function normalizedSource(row: QueryRow): FailureSource {
  return {
    id: String(row.id), storyId: String(row.story_id),
    storyRunId: row.story_run_id == null ? null : String(row.story_run_id),
    kind: String(row.kind), verdict: String(row.verdict ?? ''),
    summary: row.summary == null ? null : String(row.summary),
    sha: row.sha == null ? null : String(row.sha), detail: detailObject(row.detail),
  }
}

export function requireArtifactId(id: string): void {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id)) {
    throw new TriageInputError('An artifact UUID is required. Use list to find one.')
  }
}

export async function listTriageSources(q: QueryExecutor, storyId: string | null = null) {
  const rows = await q`
    select id, story_id, story_run_id, kind, verdict, summary, sha, created_at
    from forge_tool_artifact
    where kind in ('qa-assay-evidence', 'architecture-security', 'run-verdict')
      and lower(verdict) in ('fail', 'failed', 'error', 'hold', 'interrupted')
      and (${storyId}::text is null or story_id = ${storyId})
    order by created_at desc, id desc limit 30
  `
  return rows.map(row => ({
    ...normalizedSource(row), summary: triageText(row.summary, 500),
    createdAt: row.created_at instanceof Date ? row.created_at.toISOString() : String(row.created_at),
  }))
}

export async function readTriageSource(q: QueryExecutor, id: string): Promise<FailureSource> {
  requireArtifactId(id)
  const rows = await q`select id, story_id, story_run_id, kind, verdict, summary, detail, sha
    from forge_tool_artifact where id = ${id}::uuid`
  if (!rows[0]) throw new TriageInputError('Artifact not found.')
  const source = normalizedSource(rows[0])
  if (!isFailureSource(source.kind, source.verdict)) throw new TriageInputError('Artifact is not a supported failure source.')
  return source
}

export async function saveTriageObservation(q: QueryExecutor, source: FailureSource, observation: TriageObservation) {
  const result = await recordToolArtifact({
    storyId: source.storyId, storyRunId: source.storyRunId, sha: source.sha,
    tool: 'typesafe-triage', kind: TRIAGE_KIND,
    // Never PASS/FAIL, never a replacement for the source's verdict.
    verdict: null,
    summary: `Advisory: ${observation.suggestedClass}; confidence ${observation.confidence.toFixed(3)}; ${observation.needsReview ? 'uncertain' : 'awaiting confirmation'}`,
    detail: { ...observation },
  }, q)
  return result.id
}

export async function reviewTriage(q: QueryExecutor, id: string, confirmedClass: ForgeFailureClass, note: string) {
  requireArtifactId(id)
  if (!isForgeFailureClass(confirmedClass)) throw new TriageInputError('Unknown failure class.')
  if (!note.trim()) throw new TriageInputError('Include a short note describing the evidence for the confirmed class.')
  const rows = await q`select id, story_id, story_run_id, kind, detail, sha
    from forge_tool_artifact where id = ${id}::uuid and kind = ${TRIAGE_KIND} and tool = 'typesafe-triage'`
  if (!rows[0]) throw new TriageInputError('Triage observation not found.')
  const source = normalizedSource(rows[0])
  const result = await recordToolArtifact({
    storyId: source.storyId, storyRunId: source.storyRunId, sha: source.sha,
    tool: 'typesafe-triage', kind: REVIEW_KIND, verdict: null,
    summary: `Operator confirmed ${confirmedClass}`,
    detail: { triageArtifactId: id, confirmedClass, note: triageText(note, 1500) },
  }, q)
  return { reviewId: result.id, triageArtifactId: id, confirmedClass }
}

/** Latest evaluation per source/model/prompt, then latest operator review of that evaluation.
 * Repeated calls cannot inflate the sample count. No labels = no accuracy claim. */
export async function triageReport(q: QueryExecutor, storyId: string | null = null) {
  const rows = await q`
    with observations as (
      select distinct on (detail->>'sourceArtifactId') id, story_id, detail, created_at
      from forge_tool_artifact
      where kind = ${TRIAGE_KIND} and tool = 'typesafe-triage'
        and detail->>'model' = ${TRIAGE_MODEL}
        and detail->>'promptVersion' = ${TRIAGE_PROMPT_VERSION}
        and (${storyId}::text is null or story_id = ${storyId})
      order by detail->>'sourceArtifactId', created_at desc, id desc
    )
    select o.id, o.story_id, o.detail, r.detail as review
    from observations o left join lateral (
      select detail from forge_tool_artifact
      where kind = ${REVIEW_KIND} and tool = 'typesafe-triage'
        and detail->>'triageArtifactId' = o.id::text
      order by created_at desc, id desc limit 1
    ) r on true
    order by o.created_at desc, o.id desc limit 200
  `
  return summarizeTriageRows(rows)
}

export function summarizeTriageRows(rows: QueryRow[]) {
  const cases = rows.map(row => {
    const d = detailObject(row.detail)
    const review = detailObject(row.review)
    const confirmedClass = isForgeFailureClass(review.confirmedClass) ? review.confirmedClass : null
    return {
      id: String(row.id), storyId: String(row.story_id), sourceArtifactId: d.sourceArtifactId,
      suggestedClass: d.suggestedClass, confirmedClass,
      agrees: confirmedClass === null ? null : confirmedClass === d.suggestedClass,
      needsReview: d.needsReview === true, confidence: d.confidence,
      inputTokens: Number(d.inputTokens ?? 0), elapsedMs: Number(d.elapsedMs ?? 0),
    }
  })
  const reviewed = cases.filter(c => c.agrees !== null)
  const agreed = reviewed.filter(c => c.agrees === true).length
  return {
    model: TRIAGE_MODEL, promptVersion: TRIAGE_PROMPT_VERSION,
    scope: 'Latest evaluation per source, up to 200 sources; observational sample, not a benchmark.',
    total: cases.length, reviewed: reviewed.length, unreviewed: cases.length - reviewed.length,
    uncertain: cases.filter(c => c.needsReview).length,
    agreement: reviewed.length ? agreed / reviewed.length : null,
    inputTokensForDisplayedEvaluations: cases.reduce((sum, c) => sum + c.inputTokens, 0),
    averageElapsedMs: cases.length ? cases.reduce((sum, c) => sum + c.elapsedMs, 0) / cases.length : null,
    cases,
  }
}
