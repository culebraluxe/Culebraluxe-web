import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

// ---------------------------------------------------------------------------
// ENG-FORGE-DOC-QA-RULE-01 — THE ARCHITECTURE DOC STATES THE QA RULE THE MACHINE
// ACTUALLY IMPLEMENTS: THE VERDICT IS PASS OR FAIL, AND NO SHA RIDES ALONG WITH IT.
//
// QA has no relationship to git (see forge-qa-no-git.test.ts). The workflow
// architecture doc is read by every lane, so a stale sentence that tells a reader
// to "compare the frozen candidateSha against what QA verified" resurrects a check
// that no longer exists. Prose cannot stop that wording from coming back; this
// fence reads the document AS TEXT and fails when it does.
//
// It is PHRASE-SCOPED on purpose: the corrected doc legitimately says "no sha",
// "QA carries no git identity" and "the verdict is PASS or FAIL", so a bare
// /sha/i scan would be red on the very tree this guard is meant to protect.
// ---------------------------------------------------------------------------

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')

/** The durable explainer every lane reads. */
const DOC_PATH = 'docs/agent/WORKFLOW-ARCHITECTURE.md'

export type QaDocRule =
  | 'qa_freezes_sha'
  | 'candidate_sha_field'
  | 'sha_carried_into_release'
  | 'qa_verifies_sha'
  | 'qa_verifies_exact_candidate'

export type QaDocViolation = { rule: QaDocRule; match: string }

const RULES: ReadonlyArray<readonly [QaDocRule, RegExp]> = [
  // "QA PASS -> candidate SHA frozen" / "a QA pass freezes a candidate sha"
  [
    'qa_freezes_sha',
    /QA\s+PASS\b[^\n]{0,40}\bSHA\b[^\n]{0,20}\bfrozen\b|\bcandidate\s+SHA\s+frozen\b|\bQA\s+pass\s+freezes\s+a\s+candidate\s+sha\b/i,
  ],
  // The deleted field name itself.
  ['candidate_sha_field', /\bcandidateSha\b/],
  // "carried as process state ... through QA, into release" / "publishes THAT SHA"
  [
    'sha_carried_into_release',
    /carried\s+as\s+process\s+state[^\n]{0,80}\b(?:QA|release)\b|\bpublishes\s+\*{0,2}THAT\s+SHA\b/i,
  ],
  // "QA verifies **that** SHA" / "QA evidence against that SHA"
  ['qa_verifies_sha', /QA\s+verifies\b[^\n]{0,40}\bSHA\b|\bQA\s+evidence\s+against\s+that\s+SHA\b/i],
  // "QA / Assay exact-candidate gate" / "verification of the exact candidate ..."
  [
    'qa_verifies_exact_candidate',
    /QA\s*\/\s*Assay\s+exact-candidate|\bverif(?:y|ies|ication of)\s+the\s+exact\s+candidate\b/i,
  ],
]

/** THE PURE DETECTOR: one violation per rule found in the document's text. */
export function findQaDocRuleViolations(text: string): QaDocViolation[] {
  const violations: QaDocViolation[] = []
  for (const [rule, pattern] of RULES) {
    const match = text.match(pattern)
    if (match) violations.push({ rule, match: match[0] })
  }
  return violations
}

// --- the negative that would fail a naive guard: the corrected prose ----------

test('the guard does not trip on the rule the machine actually implements', () => {
  const prose = [
    'QA carries no git identity and freezes no commit; its verdict is PASS or FAIL over the frozen proofs.',
    'DEV_OPS publishes the candidate only when that verdict passed.',
    'Nothing recomputes "the current candidate", because "current" is a moving target.',
  ].join('\n')
  assert.deepEqual(findQaDocRuleViolations(prose), [])
})

// --- the positives: "fails if that wording returns" ---------------------------

test('the guard fails if a QA pass is said to freeze a candidate sha', () => {
  const violations = findQaDocRuleViolations('QA PASS -> candidate SHA frozen')
  assert.ok(
    violations.some((v) => v.rule === 'qa_freezes_sha'),
    `a frozen candidate sha must be a violation, got ${JSON.stringify(violations)}`,
  )
})

test('the guard fails if the candidateSha field returns', () => {
  const violations = findQaDocRuleViolations('`candidateSha` is carried as process state from Smith.')
  assert.ok(
    violations.some((v) => v.rule === 'candidate_sha_field'),
    `candidateSha must be a violation, got ${JSON.stringify(violations)}`,
  )
})

test('the guard fails if the sha is said to be carried into release', () => {
  const violations = findQaDocRuleViolations('DEV_OPS publishes THAT SHA')
  assert.ok(
    violations.some((v) => v.rule === 'sha_carried_into_release'),
    `publishing THAT SHA must be a violation, got ${JSON.stringify(violations)}`,
  )
})

test('the guard fails if QA is said to verify a sha', () => {
  const violations = findQaDocRuleViolations('QA verifies **that** SHA; DEV_OPS publishes it.')
  assert.ok(
    violations.some((v) => v.rule === 'qa_verifies_sha'),
    `QA verifying a sha must be a violation, got ${JSON.stringify(violations)}`,
  )
})

test('the guard fails if QA is said to verify the exact candidate', () => {
  const violations = findQaDocRuleViolations(
    'Assay — verification of the exact candidate against the story\u2019s frozen proofs.',
  )
  assert.ok(
    violations.some((v) => v.rule === 'qa_verifies_exact_candidate'),
    `verifying the exact candidate must be a violation, got ${JSON.stringify(violations)}`,
  )
})

// --- the live scan: TRUE on the current tree ----------------------------------

test('the architecture doc states the QA rule the machine implements', () => {
  const violations = findQaDocRuleViolations(readFileSync(join(repoRoot, DOC_PATH), 'utf8'))
  assert.deepEqual(
    violations,
    [],
    `${DOC_PATH} must not describe the deleted sha-era QA invariant: ${JSON.stringify(violations)}`,
  )
})
