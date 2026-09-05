import type { ForgeArchitectContract } from './forge-architect-contract'
import { architectContractViolations } from './forge-architect-contract'
import type { ForgeVisibilitySnapshot } from './forge-visibility'

export type ForgeMorningDecision = 'accept' | 'revise' | 'reject'

export type ForgeMorningPack = {
  storyId: string
  candidateSha: string | null
  shaEquality: ForgeVisibilitySnapshot['shaEquality']
  filesTouched: string[]
  contractViolations: string[]
  spendUsd: number | null
  spendCapUsd: number | null
  overSpendCap: boolean
  humanActions: ForgeMorningDecision[]
  readyForHumanReview: boolean
}

export function buildForgeMorningPack(input: {
  snapshot: Pick<ForgeVisibilitySnapshot, 'storyId' | 'shaChain' | 'shaEquality'>
  contract: ForgeArchitectContract | null
  filesTouched: string[]
  spendUsd?: number | null
  spendCapUsd?: number | null
}): ForgeMorningPack {
  const violations = input.contract
    ? architectContractViolations(input.filesTouched, input.contract)
    : []
  const spendUsd = input.spendUsd ?? null
  const spendCapUsd = input.spendCapUsd ?? null
  const overSpendCap =
    spendUsd !== null && spendCapUsd !== null && spendUsd > spendCapUsd
  const hasSha = Boolean(input.snapshot.shaChain.candidateSha)
  return {
    storyId: input.snapshot.storyId,
    candidateSha: input.snapshot.shaChain.candidateSha,
    shaEquality: input.snapshot.shaEquality,
    filesTouched: input.filesTouched,
    contractViolations: violations,
    spendUsd,
    spendCapUsd,
    overSpendCap,
    humanActions: ['accept', 'revise', 'reject'],
    readyForHumanReview: hasSha && violations.length === 0 && !overSpendCap,
  }
}

export function formatForgeMorningPack(pack: ForgeMorningPack): string {
  const lines = [
    `Forge morning pack ${pack.storyId}`,
    `candidate ${pack.candidateSha ?? '(none)'}`,
    `qa==candidate ${pack.shaEquality.candidateEqualsQa}`,
    `files ${pack.filesTouched.join(', ') || '(none)'}`,
    pack.contractViolations.length
      ? `OUT OF SCOPE ${pack.contractViolations.join(', ')}`
      : 'scope ok',
    pack.spendUsd !== null
      ? `spend $${pack.spendUsd}${pack.spendCapUsd !== null ? ` / cap $${pack.spendCapUsd}` : ''}`
      : 'spend unknown',
    pack.readyForHumanReview ? 'READY for accept/revise/reject' : 'NOT ready — do not treat as shipped',
  ]
  return lines.join('\n')
}
