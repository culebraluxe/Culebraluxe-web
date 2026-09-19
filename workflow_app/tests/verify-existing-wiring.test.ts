import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { promisify } from 'node:util'

import { commitOnGitBaseRef } from '../forge/agents/architect/exists-git'
import { buildLeadRoutingContext } from '../forge/lead-routing-context'

const execFileAsync = promisify(execFile)
const repoFile = (rel: string) => new URL(`../../${rel}`, import.meta.url)

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

test('verify-existing: the routing context supplies the observed candidate, and omits it when nothing was observed', () => {
  const withObservation = buildLeadRoutingContext({
    story,
    capabilities,
    observedCandidate: { sha: 'a'.repeat(40), onBaseRef: true },
  })
  assert.deepEqual(withObservation.existingCandidate, { sha: 'a'.repeat(40), onBaseRef: true })

  // Absent stays absent: the validator must see "nothing to verify" rather than a null that reads as one.
  const without = buildLeadRoutingContext({ story, capabilities })
  assert.equal('existingCandidate' in without, false)
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

test('verify-existing: production really supplies it, calls the arrangement, and the workflow has a branch', () => {
  // THE STRUCTURAL GUARD, and the point of this file. A helper that exists and is never called passes every
  // unit test it has — that is precisely how this defect survived a green suite. These assertions fail if
  // the wiring is removed, which is the only thing that keeps it wired.
  const runner = readFileSync(repoFile('workflow_app/forge/agent-runtime-role-runner.ts'), 'utf8')
  assert.equal(/observedCandidate \? \{ observedCandidate \}/.test(runner), true, 'the context is not fed')
  assert.equal(
    runner.includes('commitOnGitBaseRef(process.cwd())(storyBaseCommit, recordedCandidateSha)'),
    true,
    'ancestry check not invoked',
  )
  assert.equal(/verifyExistingArrangement\(routingReview, leadRoutingContext\.allowedProofs\)/.test(runner), true, 'the arrangement helper is not called in production')

  const workflow = readFileSync(repoFile('workflow_app/definitions/FORGE_SDLC-v6.xml'), 'utf8')
  assert.equal(/leadDecision == 'ASSAY'/.test(workflow), true, 'no ASSAY condition in the execution-shape decision')
  assert.equal(/transition name="assay" to="qa_verify"/.test(workflow), true, 'no ASSAY branch to the verification node')
})
