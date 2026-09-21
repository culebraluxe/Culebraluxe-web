import assert from 'node:assert/strict'
import test from 'node:test'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'
import { FORGE_FAILURE_CLASSES } from '@/legacy/workflow_app/forge/failure-classifier'
import {
  TRIAGE_MODEL, TRIAGE_KIND, REVIEW_KIND, analyzeFailure, parseTriageResponse,
  prepareFailureState, requestTypeSafe, triageRequest, triageText, type FailureSource,
} from '@/legacy/workflow_app/forge/typesafe-failure-triage'
import {
  readTriageSource, saveTriageObservation, reviewTriage, summarizeTriageRows, triageReport,
} from '@/legacy/db/forge-typesafe-triage'
import { runTriageCommand, validateTriageArgs } from '@/scripts/forge-triage'

const SOURCE_ID = '11111111-1111-4111-8111-111111111111'
const TRIAGE_ID = '22222222-2222-4222-8222-222222222222'
const source: FailureSource = {
  id: SOURCE_ID, storyId: 'EXAMPLE', storyRunId: null, sha: 'abc1234',
  kind: 'qa-assay-evidence', verdict: 'FAIL', summary: 'Database connection refused',
  detail: {
    failureCode: 'COMMAND_FAILED',
    commandResults: [{ command: 'node check.ts', exitCode: 1, stderrTail: 'ECONNREFUSED 127.0.0.1:5432' }],
    privateData: 'never-send-this',
  },
}

function response() {
  return {
    model: TRIAGE_MODEL,
    answers: {
      cause: {
        type: 'choice', choice: 'ENVIRONMENT_FAILURE', confidence: 0.92,
        probabilities: Object.fromEntries(FORGE_FAILURE_CLASSES.map(k => [k, k === 'ENVIRONMENT_FAILURE' ? 1 : 0])),
      },
      sufficient: { type: 'noul', noul: 0.95 },
    },
    usage: { input_tokens: 1000, output_tokens: 50 },
  }
}

function database(replies: QueryRow[][]) {
  const calls: { sql: string; values: unknown[] }[] = []
  const q: QueryExecutor = async (strings, ...values) => {
    calls.push({ sql: strings.join('?'), values })
    const reply = replies.shift()
    if (!reply) throw new Error('Unexpected database call')
    return reply
  }
  return { q, calls }
}

function sourceRow() {
  return { id: source.id, story_id: source.storyId, story_run_id: source.storyRunId,
    kind: source.kind, verdict: source.verdict, sha: source.sha, summary: source.summary, detail: source.detail }
}

test('input is allowlisted, bounded, redacted before truncation, and leaves source untouched', () => {
  const before = JSON.stringify(source)
  const state = prepareFailureState(source)
  assert.equal(state.commands[0].exitCode, 1)
  assert.equal(JSON.stringify(state).includes('never-send-this'), false)
  assert.equal(JSON.stringify(source), before)
  const secret = 'TYPESAFE_API_KEY="secret with spaces" postgres://user:pass@db/test Bearer abc.def sk-abcdefghijk'
  const redacted = triageText(secret)
  for (const value of ['secret with spaces', 'user:pass', 'abc.def', 'sk-abcdefghijk']) assert.ok(!redacted.includes(value))
  assert.equal(triageText('PASSWORD=' + 'x'.repeat(5000), 20), '[redacted]')
  assert.equal(triageText('x'.repeat(3000)).length, 1500)
})

test('success and empty evidence never reach the provider', async () => {
  let called = false
  const ports = { evaluate: async () => { called = true; return response() }, save: async () => TRIAGE_ID }
  await assert.rejects(analyzeFailure({ ...source, verdict: 'PASS' }, ports), /failed QA/)
  await assert.rejects(analyzeFailure({ ...source, summary: '', detail: {} }, ports), /no explanatory/)
  assert.equal(called, false)
})

test('rejects invalid provider class, probability, usage, model and question type', () => {
  const mutations = [
    (r: ReturnType<typeof response>) => { r.answers.cause.choice = 'GO_DEPLOY' },
    (r: ReturnType<typeof response>) => { r.answers.cause.confidence = NaN },
    (r: ReturnType<typeof response>) => { delete r.answers.cause.probabilities.UNKNOWN },
    (r: ReturnType<typeof response>) => { r.answers.cause.probabilities.UNKNOWN = 2 },
    (r: ReturnType<typeof response>) => { r.usage.input_tokens = -1 },
    (r: ReturnType<typeof response>) => { r.model = 'unverified-model' },
    (r: ReturnType<typeof response>) => { r.answers.sufficient.type = 'choice' },
    (r: ReturnType<typeof response>) => { r.answers.cause.choice = 'UNKNOWN' },
  ]
  for (const mutate of mutations) {
    const raw = response(); mutate(raw)
    assert.throws(() => parseTriageResponse(raw), /invalid triage response/)
  }
})

test('low confidence and insufficient evidence flag review independently', () => {
  assert.equal(parseTriageResponse(response()).needsReview, false)
  const low = response(); low.answers.cause.confidence = 0.2
  assert.equal(parseTriageResponse(low).needsReview, true)
  const thin = response(); thin.answers.sufficient.noul = 0.2
  assert.equal(parseTriageResponse(thin).needsReview, true)
  const unknown = response()
  unknown.answers.cause.choice = 'UNKNOWN'
  unknown.answers.cause.probabilities.ENVIRONMENT_FAILURE = 0
  unknown.answers.cause.probabilities.UNKNOWN = 1
  assert.equal(parseTriageResponse(unknown).needsReview, true)
})

test('observation retains identity and telemetry without altering source evidence', async () => {
  const before = JSON.stringify(source)
  const db = database([[{ id: TRIAGE_ID }]])
  const result = await analyzeFailure(source, {
    evaluate: async req => { assert.equal(req.model, TRIAGE_MODEL); return response() },
    save: (s, o) => saveTriageObservation(db.q, s, o),
  })
  assert.equal(result.id, TRIAGE_ID)
  assert.equal(result.sourceArtifactId, SOURCE_ID)
  assert.match(result.inputHash, /^[a-f0-9]{64}$/)
  assert.equal(result.inputTokens, 1000)
  assert.equal(result.mode, 'advisory')
  assert.equal(JSON.stringify(source), before)
  assert.equal(db.calls.length, 1)
  assert.match(db.calls[0].sql, /insert into forge_tool_artifact/)
  assert.ok(db.calls[0].values.includes(TRIAGE_KIND))
  assert.equal(db.calls[0].values[4], null) // no replacement verdict
})

test('provider and persistence failures surface; provider failure writes no observation', async () => {
  let saved = false
  await assert.rejects(analyzeFailure(source, {
    evaluate: async () => { throw new Error('provider unavailable') },
    save: async () => { saved = true; return TRIAGE_ID },
  }), /provider unavailable/)
  assert.equal(saved, false)
  await assert.rejects(analyzeFailure(source, {
    evaluate: async () => response(), save: async () => { throw new Error('storage unavailable') },
  }), /storage unavailable/)
})

test('HTTP uses pinned endpoint, key only in header, and no retry on 429', async () => {
  let count = 0
  const fake: typeof fetch = async (url, init) => {
    count++
    assert.equal(url, 'https://api.typesafe.ai/v1/systemone')
    assert.equal((init?.headers as Record<string, string>).Authorization, 'Bearer test-key')
    assert.ok(!String(init?.body).includes('test-key'))
    assert.ok(init?.signal)
    return new Response('private provider error', { status: 429 })
  }
  await assert.rejects(requestTypeSafe(triageRequest(source), 'test-key', fake), /HTTP 429/)
  assert.equal(count, 1)
  await assert.rejects(requestTypeSafe(triageRequest(source), '', fake), /TYPESAFE_API_KEY/)
  assert.equal(count, 1)
})

test('HTTP timeout aborts the request', async () => {
  const fake: typeof fetch = async (_url, init) => new Promise((_resolve, reject) => {
    const keepAlive = setTimeout(() => reject(new Error('abort did not fire')), 500)
    init?.signal?.addEventListener('abort', () => { clearTimeout(keepAlive); reject(new Error('timed out')) })
  })
  await assert.rejects(requestTypeSafe(triageRequest(source), 'test-key', fake, 5), /timed out/)
})

test('repository normalizes JSON detail and rejects nonexistent or successful sources', async () => {
  const db = database([[{ ...sourceRow(), detail: JSON.stringify(source.detail) }]])
  assert.deepEqual(await readTriageSource(db.q, SOURCE_ID), source)
  await assert.rejects(readTriageSource(database([[]]).q, SOURCE_ID), /not found/)
  await assert.rejects(readTriageSource(database([[{ ...sourceRow(), verdict: 'PASS' }]]).q, SOURCE_ID), /not a supported/)
  await assert.rejects(readTriageSource(db.q, "' OR true"), /UUID/)
})

test('review links to the observation, requires confirmed evidence, and leaves source alone', async () => {
  const db = database([[{ ...sourceRow(), id: TRIAGE_ID, kind: TRIAGE_KIND }], [{ id: 'review-id' }]])
  const result = await reviewTriage(db.q, TRIAGE_ID, 'ENVIRONMENT_FAILURE', 'Service was stopped; starting it fixed the same check.')
  assert.equal(result.triageArtifactId, TRIAGE_ID)
  assert.ok(db.calls[1].values.includes(REVIEW_KIND))
  assert.equal(db.calls[1].values[4], null)
  await assert.rejects(reviewTriage(db.q, TRIAGE_ID, 'UNKNOWN', ''), /short note/)
})

test('report excludes unreviewed cases from agreement and keeps empty agreement null', async () => {
  assert.equal(summarizeTriageRows([]).agreement, null)
  const rows = [
    { id: '1', story_id: 'S', detail: { suggestedClass: 'BAD_IMPLEMENTATION', inputTokens: 100 }, review: { confirmedClass: 'BAD_IMPLEMENTATION' } },
    { id: '2', story_id: 'S', detail: { suggestedClass: 'UNKNOWN', needsReview: true }, review: { confirmedClass: 'WEAK_TEST' } },
    { id: '3', story_id: 'S', detail: { suggestedClass: 'UNKNOWN' }, review: null },
  ]
  const result = summarizeTriageRows(rows)
  assert.equal(result.agreement, 0.5)
  assert.equal(result.reviewed, 2)
  assert.equal(result.unreviewed, 1)
  assert.equal(result.uncertain, 1)
  const db = database([rows]); await triageReport(db.q)
  assert.match(db.calls[0].sql, /distinct on/)
  assert.ok(db.calls[0].values.includes(TRIAGE_MODEL))
})

test('CLI validates arguments and preview works without an API key or any writes', async () => {
  assert.equal(validateTriageArgs([]), 'help')
  assert.throws(() => validateTriageArgs(['analyze']), /Forge TypeSafe/)
  assert.throws(() => validateTriageArgs(['review', TRIAGE_ID, 'INVALID', 'note']), /Forge TypeSafe/)
  const db = database([[sourceRow()]])
  assert.deepEqual(await runTriageCommand(['preview', SOURCE_ID], db.q, ''), triageRequest(source))
  assert.equal(db.calls.length, 1)
  assert.match(db.calls[0].sql, /^select/)
  await assert.rejects(runTriageCommand(['analyze', SOURCE_ID], db.q, ''), /TYPESAFE_API_KEY/)
  assert.equal(db.calls.length, 1)
})
