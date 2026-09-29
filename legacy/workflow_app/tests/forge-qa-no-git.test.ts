import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

// ---------------------------------------------------------------------------
// ENG-FORGE-QA-NO-GIT-GUARD-01 — QA ANSWERS ONLY "DID THE TESTS PASS".
//
// QA has no relationship to git. It runs the story's frozen proofs in the
// directory it was given and writes down what they did. That rule lives in prose
// today (qa/types.ts, qa/run.ts, assay-collect.ts), and prose cannot stop a field,
// a git call or a lineage check from being added back and silently turning QA into
// a release judge.
//
// This guard reads the QA modules AS TEXT and fails when any of them:
//   1. names a sha field      (candidateSha, verifiedSha, qaVerifiedSha, ...)
//   2. runs a git command     (child_process / execFileSync / a `git` token)
//   3. checks lineage         (merge-base, ancestor, rev-parse, ...)
//
// It is COMMENT-AWARE on purpose: the modules already SAY "No candidate, no SHA,
// no lineage" in their doc comments, and a naive /sha/i scan would be red on the
// current tree — a guard that fails before it guards anything.
// ---------------------------------------------------------------------------

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..')

/** Whole QA implementation modules whose text must stay git-free. */
const QA_MODULE_FILES = [
  'legacy/workflow_app/forge/agents/qa/run.ts',
  'legacy/workflow_app/forge/agents/qa/types.ts',
  'legacy/workflow_app/forge/agents/assay-collect.ts',
  // ADDED 2026-09-19 after the gap was measured: the receipt projection is QA-side, and it was outside this
  // list. It is clean today; listing it means it stays that way.
  'legacy/workflow_app/forge/agents/gate-checks.ts',
]

/**
 * A GIT-BEARING FILE NAMED LIKE A QA FILE IS THE HOLE THIS GUARD COULD NOT SEE (2026-09-19).
 *
 * Measured: a review demanded that QA "measure the code the route identified", and a new module
 * `legacy/workflow_app/forge/agents/assay-measurement.ts` appeared with `git rev-parse`, `merge-base --is-ancestor`
 * and a `measuredSha` field. This guard stayed GREEN, because its list named three files and the new one was
 * not among them — exactly the failure it was written to prevent ("prose cannot stop a git call from being
 * added back"). It now scans every module in the QA agent directory whose NAME says QA or Assay, so a new file
 * in that family is covered the day it exists rather than the day somebody remembers to list it.
 */
const QA_AGENT_DIR = 'legacy/workflow_app/forge/agents'
const QA_NAMED = /(^|\/)(qa|assay|gate)[^/]*\.ts$|^legacy\/workflow_app\/forge\/agents\/qa\//i

function qaAgentDirFiles(): string[] {
  const dir = join(repoRoot, QA_AGENT_DIR)
  return readdirSync(dir)
    .filter((name) => name.endsWith('.ts') && /^(qa|assay|gate)/i.test(name))
    .map((name) => `${QA_AGENT_DIR}/${name}`)
    .concat(
      readdirSync(join(dir, 'qa'))
        .filter((name) => name.endsWith('.ts'))
        .map((name) => `${QA_AGENT_DIR}/qa/${name}`),
    )
}

/** The QA phase agent, scanned as a class slice — role-agents.ts also holds DevOps/Smith. */
const QA_AGENT_FILE = 'legacy/workflow_app/forge/agents/role-agents.ts'
const QA_AGENT_CLASS = 'export class QAAgent'

export type QaGitRule = 'sha_field' | 'git_command' | 'lineage'
export type QaGitViolation = { label: string; rule: QaGitRule; match: string }

/** Read CODE, never the prose that NAMES the rule. */
export function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '')
}

const RULES: ReadonlyArray<readonly [QaGitRule, RegExp]> = [
  // A sha FIELD: bare `sha`/`SHA`, camelCase `...Sha`, and the named git-identity fields.
  ['sha_field', /\b(?:sha|commitHash|baseRef|mergeBase|[a-z][A-Za-z0-9]*[Ss][Hh][Aa])\b/i],
  // A git INVOCATION: a child_process surface or a bare `git` token (word-boundary safe: "digit" is not `git`).
  ['git_command', /\b(?:child_process|execFileSync|execFile|execSync|spawnSync|spawn)\b|\bgit\b/i],
  // A lineage CHECK.
  ['lineage', /\b(?:merge-base|mergeBase|ancestor|isAncestor|rev-parse|rev-list|descendant)\b/i],
]

/** THE PURE DETECTOR: one violation per rule found in the module's CODE. */
export function findQaGitViolations(source: string, label: string): QaGitViolation[] {
  const code = stripComments(source)
  const violations: QaGitViolation[] = []
  for (const [rule, pattern] of RULES) {
    const match = code.match(pattern)
    if (match) violations.push({ label, rule, match: match[0] })
  }
  return violations
}

/** The QAAgent class body, so DevOpsAgent's legitimate `deployedSha` is never scanned. */
export function qaAgentSlice(source: string): string {
  const start = source.indexOf(QA_AGENT_CLASS)
  assert.ok(start >= 0, `${QA_AGENT_FILE} must still define ${QA_AGENT_CLASS}`)
  const rest = source.slice(start)
  const next = rest.slice(QA_AGENT_CLASS.length).search(/\nexport class /)
  return next < 0 ? rest : rest.slice(0, QA_AGENT_CLASS.length + next)
}

// --- the negative that would fail a naive guard: prose that names the rule -----

test('the guard is comment-aware: prose that NAMES the rule does not trip it', () => {
  const prose = [
    '// Deterministic Assay. No model, no git, no promotion advice.',
    '/** What QA is given: no candidate, no SHA, no lineage — QA has no relationship to git. */',
  ].join('\n')
  assert.deepEqual(findQaGitViolations(prose, 'prose.ts'), [])
})

// --- the positives: "fails if such a reference is added back" ------------------

test('the guard fails if a sha field is added back', () => {
  const violations = findQaGitViolations('export type AssayPlan = { verifiedSha: string }', 'synthetic.ts')
  assert.ok(
    violations.some((v) => v.rule === 'sha_field'),
    `a sha field must be a violation, got ${JSON.stringify(violations)}`,
  )
})

test('the guard fails if a git command is added back', () => {
  const source = [
    "import { execFileSync } from 'node:child_process'",
    "execFileSync('git', ['rev-parse', 'HEAD'])",
  ].join('\n')
  const violations = findQaGitViolations(source, 'synthetic.ts')
  assert.ok(
    violations.some((v) => v.rule === 'git_command'),
    `a git command must be a violation, got ${JSON.stringify(violations)}`,
  )
})

test('the guard fails if a lineage check is added back', () => {
  const violations = findQaGitViolations('const ok = isAncestor(candidate, head)', 'synthetic.ts')
  assert.ok(
    violations.some((v) => v.rule === 'lineage'),
    `a lineage check must be a violation, got ${JSON.stringify(violations)}`,
  )
})

// --- the live scan: TRUE on the current tree, and it never scans tests ---------

test('the QA module set excludes test fixtures', () => {
  for (const file of QA_MODULE_FILES) {
    assert.ok(!file.includes('.test.'), `${file} is a test file and must never be scanned`)
    assert.ok(!file.startsWith('legacy/workflow_app/tests/'), `${file} must not be under workflow_app/tests/`)
  }
})

test('no QA module names a sha field, runs a git command or checks lineage', () => {
  const violations: QaGitViolation[] = []
  const files = [...new Set([...QA_MODULE_FILES, ...qaAgentDirFiles()])]
  for (const file of files) {
    violations.push(...findQaGitViolations(readFileSync(join(repoRoot, file), 'utf8'), file))
  }
  const roleAgents = readFileSync(join(repoRoot, QA_AGENT_FILE), 'utf8')
  violations.push(...findQaGitViolations(qaAgentSlice(roleAgents), `${QA_AGENT_FILE}#QAAgent`))

  assert.deepEqual(
    violations,
    [],
    `QA must not touch git or carry a sha: ${JSON.stringify(violations)}`,
  )
})

test('the directory scan really covers the QA family — a new QA-named file is included', () => {
  // The guard is only as good as its reach. This asserts the reach: every QA/Assay/gate-named module in the
  // agents directory (and the whole qa/ directory) is scanned, so the 2026-09-19 hole cannot reopen by naming
  // a file something this guard does not look at.
  const files = qaAgentDirFiles()
  assert.ok(files.includes('legacy/workflow_app/forge/agents/assay-collect.ts'), files.join(', '))
  assert.ok(files.includes('legacy/workflow_app/forge/agents/gate-checks.ts'), files.join(', '))
  assert.ok(files.includes('legacy/workflow_app/forge/agents/qa/run.ts'), files.join(', '))
  for (const file of files) {
    assert.ok(QA_NAMED.test(file), `${file} is scanned but not QA-named — the reach rule is inconsistent`)
  }
})
