import assert from 'node:assert/strict'
import test from 'node:test'
import {
  architectContractFromNotes,
  architectContractViolations,
  parseForgeArchitectContract,
} from '@/legacy/workflow_app/forge/forge-architect-contract'
import { buildForgeMorningPack } from '@/legacy/workflow_app/forge/forge-morning-pack'
import { planForgeNight } from '@/legacy/workflow_app/forge/forge-night-driver'
import { parseForgeReleaseReceipt } from '@/legacy/workflow_app/forge/forge-receipt'
import { forgeSpendShouldHold, parseForgeSpendCapUsd } from '@/legacy/workflow_app/forge/forge-spend-cap'
import { isForgeClaimStale, selectStaleForgeClaims } from '@/legacy/workflow_app/forge/forge-stale-claim'
import { forgeVisibilityEquality } from '@/legacy/workflow_app/forge/forge-visibility'

test('V12 night driver: engine skips reducer hydrate/follow/publish', () => {
  const reducer = planForgeNight('reducer')
  assert.equal(reducer.hydrate, true)
  assert.equal(reducer.driveEngine, false)
  const engine = planForgeNight('engine')
  assert.equal(engine.hydrate, false)
  assert.equal(engine.follow, false)
  assert.equal(engine.publish, false)
  assert.equal(engine.driveEngine, true)
  assert.equal(planForgeNight(undefined).brain, 'engine') // ENGINE is canonical
})

test('V12 architect contract binds Smith scope', () => {
  const contract = parseForgeArchitectContract({
    filesInScope: ['legacy/workflow_app/forge/', 'docs/agent/packets/ENG-FORGE-V12.md'],
    filesOutOfScope: ['legacy/workflow_app/real-estate/'],
    acceptance: ['night driver tests pass'],
    risk: 'dual-write',
    leadHint: 'SMITH',
  })
  assert.ok(contract)
  assert.deepEqual(
    architectContractViolations(
      ['legacy/workflow_app/forge/forge-night-driver.ts', 'legacy/workflow_app/real-estate/foo.ts'],
      contract!,
    ),
    ['legacy/workflow_app/real-estate/foo.ts'],
  )
  assert.ok(
    architectContractFromNotes(
      'FORGE_ARCHITECT_CONTRACT: {"filesInScope":["a.ts"],"leadHint":"SOLO"}',
    ),
  )
})

test('V12 morning pack is review of working SHA, not an empty gate', () => {
  const pack = buildForgeMorningPack({
    snapshot: {
      storyId: 'ENG-FORGE-V12',
      shaChain: {
        candidateSha: 'aaaaaaaa',
        qaVerifiedSha: 'aaaaaaaa',
        publishedSha: null,
        deployedSha: null,
        productionVerifiedSha: null,
      },
      shaEquality: forgeVisibilityEquality({
        candidateSha: 'aaaaaaaa',
        qaVerifiedSha: 'aaaaaaaa',
        publishedSha: null,
        deployedSha: null,
        productionVerifiedSha: null,
      }),
    },
    contract: parseForgeArchitectContract({
      filesInScope: ['legacy/workflow_app/forge/'],
      leadHint: 'SMITH',
    }),
    filesTouched: ['legacy/workflow_app/forge/forge-morning-pack.ts'],
    spendUsd: 1.2,
    spendCapUsd: 10,
  })
  assert.equal(pack.readyForHumanReview, true)
  assert.equal(pack.contractViolations.length, 0)
})

test('V12 stale claims release after threshold; ready tasks never stale', () => {
  const now = 1_000_000
  assert.equal(
    isForgeClaimStale(
      { taskId: 't1', nodeId: 'smith', status: 'in_progress', claimedAtMs: now - 61 * 60_000 },
      now,
      60 * 60_000,
    ),
    true,
  )
  assert.equal(
    isForgeClaimStale(
      { taskId: 't2', nodeId: 'smith', status: 'ready', claimedAtMs: now - 61 * 60_000 },
      now,
      60 * 60_000,
    ),
    false,
  )
  assert.equal(
    selectStaleForgeClaims(
      [{ taskId: 't3', nodeId: 'hold', status: 'reserved', claimedAtMs: now - 1000 }],
      now,
      60 * 60_000,
    ).length,
    0,
  )
})

test('V12 receipts stay provider-neutral and spend cap HOLDs', () => {
  const ok = parseForgeReleaseReceipt({
    kind: 'deployment',
    artifactSha: 'abcdef0',
    receiptId: 'rcp_1',
    success: true,
    provider: 'edge-host',
  })
  assert.equal(ok?.artifactSha, 'abcdef0')
  assert.equal(parseForgeReleaseReceipt({ kind: 'deployment', artifactSha: 'nope', receiptId: 'x' }), null)
  assert.equal(parseForgeSpendCapUsd('25'), 25)
  assert.equal(forgeSpendShouldHold({ spendUsd: 26, capUsd: 25 }), true)
  assert.equal(forgeSpendShouldHold({ spendUsd: 10, capUsd: null }), false)
})
