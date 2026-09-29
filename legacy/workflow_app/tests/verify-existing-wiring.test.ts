import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { commitOnGitBaseRef } from '@/legacy/workflow_app/forge/agents/architect/exists-git'
import { buildLeadRoutingContext } from '@/legacy/workflow_app/forge/lead-routing-context'
import { leadProposalFromFields, resolveLeadProposal } from '@/legacy/workflow_app/forge/lead-proposal-resolve'
import { buildLeadRoutingDirective } from '@/legacy/workflow_app/forge/forge-lead-routing-prompt'
import { LEAD_DECISION, VERIFY_CANDIDATE, mediateField } from '@/lib/field-mediator'
import { validateLeadDecisionWrite } from '@/lib/lead-decision-write'
import type { ForgeRoleContract } from '@/legacy/db/forge-role-contract'

const execFileAsync = promisify(execFile)
const repoFile = (rel: string) => new URL(`../../../${rel}`, import.meta.url)

async function git(cwd: string, args: string[]): Promise<string> {
  const { stdout } = await execFileAsync('git', args, { cwd, encoding: 'utf8' })
  return stdout.trim()
}

// ---------------------------------------------------------------------------
// FORGE-VERIFY-EXISTING-COMPLETE-01 — the direct-to-QA route is WIRED, not merely fenced.
//
// Astra review 1.5, measured: the ASSAY proposal validator and the arrangement helper both existed, the
// routing context never supplied `existingCandidate`, the arrangement helper was never called by the
// production runner, and the active workflow had no ASSAY branch — so the focused tests demonstrated
// helpers, not a working route. That is why ENG-FORGE-VERIFY-EXISTING-01 was closed on helper-grade
// evidence and reopened. This file fences the wiring itself: the supplier, the ancestry rule, and the three
// structural facts that make the route reachable.
// ---------------------------------------------------------------------------

const story = {
  id: 'STORY-1',
  assayCommands: 'node --test x.test.ts',
  acceptanceCriteria: 'the thing works',
} as never

const capabilities = { splitEnabled: true, maxSmiths: 2 } as never

test('verify-existing-git: the routing context supplies the GIT-observed candidate, and omits it when git saw nothing', () => {
  const withObservation = buildLeadRoutingContext({
    story,
    capabilities,
    gitObservedCandidate: { sha: 'a'.repeat(40), onBaseRef: true },
  })
  assert.deepEqual(withObservation.gitObservedCandidate, { sha: 'a'.repeat(40), onBaseRef: true })

  // Absent stays absent: the validator must see "nothing to verify" rather than a null that reads as one.
  const without = buildLeadRoutingContext({ story, findings: REQUIRED_FINDING, capabilities })
  assert.equal('gitObservedCandidate' in without, false)
})

test('verify-existing: ancestry is the test, not existence — a commit off the base is refused', async () => {
  const repo = await mkdtemp(join(tmpdir(), 'forge-verify-existing-'))
  try {
    await git(repo, ['init', '-b', 'main', '-q'])
    await git(repo, ['config', 'user.name', 'fence'])
    await git(repo, ['config', 'user.email', 'fence@test'])
    await git(repo, ['config', 'commit.gpgsign', 'false'])
    await writeFile(join(repo, 'a.ts'), 'export const a = 1\n')
    await git(repo, ['add', '-A'])
    await git(repo, ['commit', '-q', '-m', 'base'])
    const base = await git(repo, ['rev-parse', 'HEAD'])

    await git(repo, ['checkout', '-q', '-b', 'side'])
    await writeFile(join(repo, 'b.ts'), 'export const b = 2\n')
    await git(repo, ['add', '-A'])
    await git(repo, ['commit', '-q', '-m', 'side work'])
    const side = await git(repo, ['rev-parse', 'HEAD'])

    const onBase = commitOnGitBaseRef(repo)
    assert.equal(onBase(base, base), true, 'the base is on itself')
    assert.equal(onBase(base, side), false, 'work that exists but never landed is not "already there"')
    assert.equal(onBase(base, 'f'.repeat(40)), false, 'an unknown sha is a refusal, not a yes')
    assert.equal(onBase('no-such-ref', base), false, 'an unknown base is a refusal, not a yes')
  } finally {
    await rm(repo, { recursive: true, force: true })
  }
})

// ---------------------------------------------------------------------------
// WORK PACKAGE A — THE CONTRACT, THE DECISION, AND THE NAMED CANDIDATE.
//
// Astra's re-review measured the gap precisely: the validator, the arrangement and the workflow branch all
// existed, and the route could still never fire, because the words never reached a row. `--decision ASSAY` was
// refused by the mediator, `forge_role_contract.decision` permitted only four values, there was nowhere to
// record WHICH candidate the decision was about, and `leadProposalFromFields` dropped it anyway.
//
// These fences exercise the same functions production uses — the mediator, the handoff's decision predicate,
// the proposal builder and the routing validator — rather than their source strings.
// ---------------------------------------------------------------------------

const ASSAY_SHA = 'b'.repeat(40)

/**
 * A routing context with ONE REQUIRED FINDING, which the validator insists on before it will judge any route
 * ("No required findings supplied; obtain the bounded Architect handoff"). Without it every ASSAY case below
 * would be refused for a reason that has nothing to do with ASSAY, and the fences would prove nothing.
 */
const REQUIRED_FINDING = [{ id: 'f-1', required: true, seams: ['legacy/workflow_app/forge/'] }] as never

const contractRow = (over: Partial<ForgeRoleContract> = {}): ForgeRoleContract => ({
  decision: 'ASSAY',
  size: 'SMALL',
  sizeReason: 'the work already exists',
  reason: 'the findings are satisfied by the observed candidate',
  assignmentCount: 0,
  findingIds: ['f-1'],
  mergeChecks: ['node --test x.test.ts'],
  surfaceScope: [],
  acceptanceAssertions: null,
  verifyCandidate: ASSAY_SHA,
  attempt: 1,
  ...over,
})

test('mediator: ASSAY is accepted as a Lead decision — the closed set used to refuse the word itself', () => {
  const mediated = mediateField(LEAD_DECISION, 'assay')
  assert.equal(mediated.ok, true)
  assert.equal(mediated.ok && mediated.value, 'ASSAY')
  // The set is still CLOSED: an invented route is refused with the accepted set, not coerced.
  const refused = mediateField(LEAD_DECISION, 'REPAIR')
  assert.equal(refused.ok, false)
  assert.equal(refused.ok === false && refused.reason, 'NOT_IN_SET')
})

test('mediator: the verification candidate is EXACTLY 40 hex, refused at the write rather than later', () => {
  const short = mediateField(VERIFY_CANDIDATE, 'abc1234')
  assert.equal(short.ok, false, 'a 7-character prefix is not an identity')
  assert.equal(short.ok === false && short.reason, 'NOT_A_SHA')
  const upper = mediateField(VERIFY_CANDIDATE, ASSAY_SHA.toUpperCase())
  assert.equal(upper.ok, true)
  assert.equal(upper.ok && upper.value, ASSAY_SHA, 'normalized to lowercase, which is what the validator compares')
})

test('handoff: the decision predicate refuses each mismatch BY NAME', () => {
  assert.deepEqual(validateLeadDecisionWrite({ decision: 'ASSAY', verifyCandidate: ASSAY_SHA }), {
    ok: true,
    decision: 'ASSAY',
    verifyCandidate: ASSAY_SHA,
  })

  const missing = validateLeadDecisionWrite({ decision: 'ASSAY', verifyCandidate: null })
  assert.equal(missing.ok, false)
  assert.equal(missing.ok === false && missing.code, 'ASSAY_REQUIRES_CANDIDATE')

  const mismatch = validateLeadDecisionWrite({ decision: 'SOLO', verifyCandidate: ASSAY_SHA })
  assert.equal(mismatch.ok, false)
  assert.equal(mismatch.ok === false && mismatch.code, 'CANDIDATE_REQUIRES_ASSAY')

  const malformed = validateLeadDecisionWrite({ decision: 'ASSAY', verifyCandidate: 'not-a-sha' })
  assert.equal(malformed.ok, false)
  assert.equal(malformed.ok === false && malformed.code, 'CANDIDATE_NOT_A_SHA')

  // The four original routes are untouched.
  for (const decision of ['SOLO', 'SMITH', 'SPLIT', 'HOLD']) {
    const ok = validateLeadDecisionWrite({ decision, verifyCandidate: null })
    assert.equal(ok.ok, true, `${decision} must still be writable without a candidate`)
  }
})

test('proposal: a persisted ASSAY contract round-trips into a proposal that NAMES its candidate', () => {
  const proposal = leadProposalFromFields(contractRow(), null) as Record<string, unknown> | null
  assert.ok(proposal, 'a recorded decision must produce a proposal')
  assert.equal(proposal?.decision, 'ASSAY')
  assert.equal(proposal?.verifyCandidate, ASSAY_SHA, 'the candidate was dropped here, which made ASSAY unroutable')
  assert.deepEqual(proposal?.assignments, [], 'ASSAY dispatches no Smith')
})

test('proposal: the recorded contract routes cleanly when the runner observed the same candidate', () => {
  const context = buildLeadRoutingContext({
    story,
    findings: REQUIRED_FINDING,
    capabilities,
    gitObservedCandidate: { sha: ASSAY_SHA, onBaseRef: true },
  })
  const review = resolveLeadProposal({ contract: contractRow(), plan: null, context })
  assert.equal(review.ok, true, review.ok === false ? review.errors.join(' | ') : '')
  assert.equal(review.ok === true && review.proposal.decision, 'ASSAY')
  assert.equal(review.ok === true && review.proposal.verifyCandidate, ASSAY_SHA)
})

test('proposal: unobserved, off-base and missing candidates are each refused with their own reason', () => {
  const unobserved = resolveLeadProposal({
    contract: contractRow(),
    plan: null,
    context: buildLeadRoutingContext({ story, findings: REQUIRED_FINDING, capabilities }),
  })
  assert.equal(unobserved.ok, false)
  assert.match(unobserved.ok === false ? unobserved.errors.join(' | ') : '', /observed on the base/)

  const offBase = resolveLeadProposal({
    contract: contractRow(),
    plan: null,
    context: buildLeadRoutingContext({
      story,
      findings: REQUIRED_FINDING,
      capabilities,
      gitObservedCandidate: { sha: ASSAY_SHA, onBaseRef: false },
    }),
  })
  assert.equal(offBase.ok, false)
  assert.match(offBase.ok === false ? offBase.errors.join(' | ') : '', /not on the pinned baseRef/)

  // A contract whose candidate is MISSING cannot route either: "verify unnamed work" is not a route.
  const noCandidate = resolveLeadProposal({
    contract: contractRow({ verifyCandidate: null }),
    plan: null,
    context: buildLeadRoutingContext({
      story,
      findings: REQUIRED_FINDING,
      capabilities,
      gitObservedCandidate: { sha: ASSAY_SHA, onBaseRef: true },
    }),
  })
  assert.equal(noCandidate.ok, false)
  assert.match(noCandidate.ok === false ? noCandidate.errors.join(' | ') : '', /40-hex verifyCandidate sha/)
})

test('proposal: a non-ASSAY route that names a candidate is refused as a contradiction', () => {
  const hold = resolveLeadProposal({
    contract: contractRow({ decision: 'HOLD', verifyCandidate: null }),
    plan: null,
    context: buildLeadRoutingContext({ story, findings: REQUIRED_FINDING, capabilities }),
  })
  assert.equal(hold.ok, true, 'a plain HOLD is still a valid routing outcome')
  const contradiction = resolveLeadProposal({
    contract: contractRow({ decision: 'SOLO' }),
    plan: null,
    context: buildLeadRoutingContext({
      story,
      findings: REQUIRED_FINDING,
      capabilities,
      gitObservedCandidate: { sha: ASSAY_SHA, onBaseRef: true },
    }),
  })
  assert.equal(contradiction.ok, false)
  assert.match(contradiction.ok === false ? contradiction.errors.join(' | ') : '', /must not name a verifyCandidate/)
})

test('no-git: the QA path carries NO git identity — the route fact lives in the routing context', () => {
  // ENG-FORGE-QA-NO-GIT-GUARD-01, RESTORED. A brief version of this branch measured the primary checkout
  // (`git rev-parse HEAD`, `merge-base --is-ancestor`) and wrote `candidateSha`/`qaVerifiedSha` from it, so a
  // story could HOLD because the checkout moved rather than because a test failed — a git fact in front of QA,
  // and the QA-held sha that story removed after it refused every release. The git fact that IS legitimate is a
  // ROUTING fact: the context refuses an ASSAY whose candidate the runner never observed on the base. That is
  // where it is checked, and this fence pins both halves.
  const runner = readFileSync(repoFile('legacy/workflow_app/forge/agent-runtime-role-runner.ts'), 'utf8')
  const assayBlock = runner.slice(runner.indexOf("decision === 'ASSAY'"))
  // COMMENTS ARE STRIPPED FIRST, exactly as the repository's own no-git guard does: the block's comment NAMES
  // what was removed (`qaVerifiedSha`, `measuredSha`), and a naive scan would be red on a correct tree.
  const code = assayBlock.slice(0, 2000).replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '')
  assert.equal(
    /measuredSha|qaVerifiedSha|measureAssayCandidate/.test(code),
    false,
    'the ASSAY branch must not measure git or write a git identity',
  )
  assert.equal(
    /Object\.assign\(evidence, \{ assayRoute: arrangement \}\)/.test(runner),
    true,
    'the runner records the arrangement and nothing git-derived',
  )

  // The routing context may — and does — establish the GIT-observed candidate, and its name says so: the field
  // is `gitObservedCandidate`, so nothing that touches git shares a name with the git-free assay/QA family.
  // That naming rule is the operator's, 2026-09-19: "if there is another ASSAY that contradicts the no-git QA
  // policy it must not be called, or be named so it explicitly says Assay-With-Git".
  const routing = readFileSync(repoFile('legacy/workflow_app/forge/lead-routing-context.ts'), 'utf8')
  assert.equal(/gitObservedCandidate/.test(routing), true, 'the git fact must be named as a git fact')
  assert.equal(
    /\bobservedCandidate\b/.test(routing.replace(/gitObservedCandidate/g, '')),
    false,
    'no git-bearing field may keep an unqualified name',
  )
  const contexts = readFileSync(repoFile('legacy/workflow_app/forge/forge-lead-routing.ts'), 'utf8')
  assert.equal(/gitObservedCandidate/.test(contexts), true)
})

test('directive: ASSAY is offered only when the runner observed a candidate on the base', () => {
  const observed = { sha: ASSAY_SHA, onBaseRef: true }
  const offered = buildLeadRoutingDirective(
    buildLeadRoutingContext({ story, findings: REQUIRED_FINDING, capabilities, gitObservedCandidate: observed }) as never,
  )
  assert.match(offered, /DIRECT-TO-QA \(ASSAY\) IS AVAILABLE/)
  assert.equal(offered.includes(`--verify-candidate ${ASSAY_SHA}`), true, 'the sha must be named, not described')

  const unavailable = buildLeadRoutingDirective(buildLeadRoutingContext({ story, findings: REQUIRED_FINDING, capabilities }) as never)
  assert.match(unavailable, /ASSAY\) IS NOT AVAILABLE THIS RUN/)
  assert.equal(/--verify-candidate [0-9a-f]{40}/.test(unavailable), false, 'no sha may be suggested when none was seen')

  const offBase = buildLeadRoutingDirective(
    buildLeadRoutingContext({
      story,
      findings: REQUIRED_FINDING,
      capabilities,
      gitObservedCandidate: { sha: ASSAY_SHA, onBaseRef: false },
    }) as never,
  )
  assert.match(offBase, /NOT on the story base/)
  assert.equal(/--verify-candidate [0-9a-f]{40}/.test(offBase), false)
})

test('verify-existing: production really supplies it, calls the arrangement, and the workflow has a branch', () => {
  // THE STRUCTURAL GUARD, and the point of this file. A helper that exists and is never called passes every
  // unit test it has — that is precisely how this defect survived a green suite. These assertions fail if
  // the wiring is removed, which is the only thing that keeps it wired.
  const runner = readFileSync(repoFile('legacy/workflow_app/forge/agent-runtime-role-runner.ts'), 'utf8')
  assert.equal(/gitObservedCandidate \? \{ gitObservedCandidate \}/.test(runner), true, 'the context is not fed')
  assert.equal(
    runner.includes('commitOnGitBaseRef(process.cwd())(storyBaseCommit, recordedCandidateSha)'),
    true,
    'ancestry check not invoked',
  )
  assert.equal(/assayRouteArrangement\(routingReview, leadRoutingContext\.allowedProofs\)/.test(runner), true, 'the arrangement helper is not called in production')

  const workflow = readFileSync(repoFile('legacy/workflow_app/definitions/FORGE_SDLC-v6.xml'), 'utf8')
  assert.equal(/leadDecision == 'ASSAY'/.test(workflow), true, 'no ASSAY condition in the execution-shape decision')
  assert.equal(/transition name="assay" to="qa_verify"/.test(workflow), true, 'no ASSAY branch to the verification node')
})
