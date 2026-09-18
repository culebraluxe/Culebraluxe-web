import type { QueryExecutor, QueryRow } from './query-executor'
import type { AssayEvidence } from '../agent-runtime/assay-evidence'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export type ForgeToolArtifact = QueryRow & {
  id: string
  story_id: string
  story_run_id: string | null
  tool: string
  kind: string
  verdict: string | null
  summary: string | null
  sha: string | null
  created_at: unknown
}

/** Where a receipt's sha came from, and the defect when it came from prose instead of a column. */
export type ReceiptSha = {
  sha: string | null
  source: 'column' | 'notes' | 'absent'
  defect: string | null
}

const SHA_IN_TEXT = /\b[0-9a-f]{7,40}\b/i

/**
 * ENG-FORGE-RECEIPT-COLUMNS-01 — A RECEIPT'S SHA IS READ FROM ITS COLUMN, NEVER FROM PROSE.
 *
 * The `sha` column is what joins a receipt to its artifact. A sha that appears only inside
 * the notes/summary text is a named defect, because a reader would have to parse prose to
 * resolve it — so this resolver returns it as a defect rather than silently extracting it.
 */
export function resolveReceiptSha(input: { sha?: string | null; notes?: string | null }): ReceiptSha {
  const column = (input.sha ?? '').trim().toLowerCase()
  if (column) return { sha: column, source: 'column', defect: null }
  const found = SHA_IN_TEXT.exec(input.notes ?? '')
  if (found) {
    return {
      sha: null,
      source: 'notes',
      defect: `receipt sha ${found[0]} exists only in notes text; write it to the sha column`,
    }
  }
  return { sha: null, source: 'absent', defect: null }
}

/** Resolve a persisted artifact row's sha from its column, flagging a notes-only sha. */
export function receiptShaFromArtifact(
  row: Pick<ForgeToolArtifact, 'sha' | 'summary'>,
): ReceiptSha {
  return resolveReceiptSha({ sha: row.sha, notes: row.summary })
}

export type RecordToolArtifactInput = {
  storyId: string
  storyRunId?: string | null
  tool: string
  kind: string
  verdict?: string | null
  summary?: string | null
  /** Optional machine detail (e.g. findings list) — small, bounded, never secrets. */
  detail?: Record<string, unknown> | null
  sha?: string | null
}

/**
 * Verdicts that mean "the run came out clean". Agreement is POLARITY, not spelling:
 * `PASS` and `Complete` are the same answer in two vocabularies, while `Hold` is the
 * opposite of either.
 */
const CLEAN_ARTIFACT_VERDICTS = new Set([
  'complete',
  'pass',
  'passed',
  'success',
  'succeeded',
  'delivered',
  'done',
])

function isCleanArtifactVerdict(verdict: string): boolean {
  return CLEAN_ARTIFACT_VERDICTS.has(verdict.trim().toLowerCase())
}

/**
 * AN ARTIFACT CARRIES THE RULING, NEVER A SECOND OPINION.
 *
 * A `run-verdict` artifact is keyed to a story run whose durable ruling already exists
 * (`storyboard_story_run.result_status`, written by `finishStoryRun`). A caller-supplied
 * verdict is stored only when it agrees in polarity with that ruling; a contradicting
 * verdict becomes `null` — the artifact says it has no verdict — while summary/detail are
 * preserved. Other artifact kinds (assay evidence, static gate) ARE the ruling and keep
 * their own vocabulary, and an artifact with no run to rule it is left untouched.
 */
export function artifactVerdictForRun(input: {
  kind: string
  ruling: string | null | undefined
  verdict: string | null | undefined
}): string | null {
  const verdict = input.verdict?.trim() ?? ''
  if (!verdict) return null
  if (input.kind !== 'run-verdict') return input.verdict ?? null
  const ruling = input.ruling?.trim() ?? ''
  if (!ruling) return null
  return isCleanArtifactVerdict(ruling) === isCleanArtifactVerdict(verdict)
    ? input.verdict ?? null
    : null
}

/** One flat child row keyed to a story run (the execution unit). */
export async function recordToolArtifact(
  input: RecordToolArtifactInput,
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact> {
  const q = execute ?? (await executor())
  let verdict = input.verdict ?? null
  if (verdict !== null && input.kind === 'run-verdict' && input.storyRunId) {
    // The ruling is read from the run, not taken from the caller. A failed or empty
    // read fails closed to no verdict rather than inventing one. Both paths assign before the
    // read below, so the initialiser was dead (and it reached a full QA pass: the static gate
    // runs semgrep, knip and tsc, not eslint).
    let ruling: string | null
    try {
      const runRows = await q`
        select result_status from storyboard_story_run where id = ${input.storyRunId}
      `
      ruling = (runRows[0]?.result_status as string | null | undefined) ?? null
    } catch {
      ruling = null
    }
    verdict = artifactVerdictForRun({ kind: input.kind, ruling, verdict })
  }
  const rows = await q`
    insert into forge_tool_artifact (story_id, story_run_id, tool, kind, verdict, summary, detail, sha)
    values (
      ${input.storyId}, ${input.storyRunId ?? null}, ${input.tool}, ${input.kind},
      ${verdict}, ${input.summary ?? null},
      ${input.detail ? JSON.stringify(input.detail) : null}::jsonb, ${input.sha ?? null}
    )
    returning id, story_id, story_run_id, tool, kind, verdict, summary, sha, created_at
  `
  return rows[0] as ForgeToolArtifact
}

export async function listToolArtifactsForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, story_run_id, tool, kind, verdict, summary, sha, created_at
    from forge_tool_artifact
    where story_id = ${storyId}
    order by created_at desc
  `
  return rows as ForgeToolArtifact[]
}

export async function listToolArtifactsForRun(
  storyRunId: string,
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, story_run_id, tool, kind, verdict, summary, sha, created_at
    from forge_tool_artifact
    where story_run_id = ${storyRunId}
    order by created_at desc
  `
  return rows as ForgeToolArtifact[]
}

/**
 * Persist the QA assay verdict AND ITS REASON as a child artifact.
 *
 * The AssayEvidence already carries the complete diagnosis (verdict, failureCode,
 * failureDetail, policyViolations, per-command exit codes/tails), but until now only
 * the derived booleans (qa_passed/failure_class) reached the database — so a QA FAIL
 * could not be explained after the fact. That cost real time: WS-05 failed QA four
 * times with CODE_DEFECT while all three frozen commands passed locally in the same
 * worktree at the same SHA, and there was no durable record of WHY.
 *
 * Best-effort by design: a failed write must never change the QA verdict.
 */
export async function recordAssayEvidenceArtifact(
  input: {
    storyId: string
    storyRunId?: string | null
    evidence: AssayEvidence
  },
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact> {
  const { evidence } = input
  const reason =
    evidence.verdict === 'PASS'
      ? 'every frozen command passed'
      : evidence.failureCode ??
        evidence.policyViolations[0] ??
        evidence.failureDetail ??
        'assay failed without a recorded code'
  return recordToolArtifact(
    {
      storyId: input.storyId,
      storyRunId: input.storyRunId ?? null,
      tool: 'assay',
      kind: 'qa-assay-evidence',
      verdict: evidence.verdict,
      summary: `${evidence.verdict}: ${reason}`.slice(0, 1000),
      detail: {
        verdict: evidence.verdict,
        failureCode: evidence.failureCode,
        failureDetail: evidence.failureDetail,
        candidateSha: evidence.candidateSha,
        verifiedSha: evidence.verifiedSha,
        candidateMatchesVerified: evidence.candidateSha === evidence.verifiedSha,
        requiredCommands: evidence.requiredCommands,
        policyViolations: evidence.policyViolations.slice(0, 8),
        commandResults: evidence.commandResults.slice(0, 12).map((result) => ({
          command: result.command,
          exitCode: result.exitCode,
          signal: result.signal,
          timedOut: result.timedOut,
          durationMs: result.durationMs,
          tests: result.tests,
          stderrTail: result.stderrTail.slice(-400),
        })),
        startedAt: evidence.startedAt,
        endedAt: evidence.endedAt,
      },
      sha: evidence.verifiedSha ?? evidence.candidateSha ?? null,
    },
    execute,
  )
}

/** Persist a static-gate verdict (from runStaticGate) as a child artifact. */
export async function recordStaticGateArtifact(
  input: {
    storyId: string
    storyRunId?: string | null
    sha?: string | null
    archOk: boolean
    archErrorCount: number
    semgrepRan: boolean
    semgrepFindingCount: number
    /** Hygiene instrument (V5-27). Optional so existing callers keep working. */
    knipRan?: boolean
    knipFindingCount?: number
  },
  execute?: QueryExecutor,
): Promise<ForgeToolArtifact> {
  const summary =
    `${input.archOk ? 'architecture clean' : `${input.archErrorCount} architecture violation(s)`}` +
    (input.semgrepRan
      ? `; semgrep ${input.semgrepFindingCount === 0 ? 'clean' : `${input.semgrepFindingCount} finding(s)`}`
      : '') +
    (input.knipRan
      ? `; knip ${input.knipFindingCount === 0 ? 'clean' : `${input.knipFindingCount} finding(s)`}`
      : '')
  return recordToolArtifact(
    {
      storyId: input.storyId,
      storyRunId: input.storyRunId ?? null,
      tool: 'static-gate',
      kind: 'architecture-security',
      verdict: input.archOk ? 'PASS' : 'FAIL',
      summary,
      detail: {
        archOk: input.archOk,
        archErrorCount: input.archErrorCount,
        semgrepRan: input.semgrepRan,
        semgrepFindingCount: input.semgrepFindingCount,
      },
      sha: input.sha ?? null,
    },
    execute,
  )
}
