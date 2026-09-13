/** ADD. Applied by LeadAgent before the live reviewLeadProposal result is written. */
export type BenchIntent = 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | null

export function benchIntentErrors(decision: string, benchIntent: BenchIntent | undefined): string[] {
  if (!benchIntent) return []
  if (benchIntent === 'HOLD' && decision !== 'HOLD') return ['Bench intent is HOLD — Lead may only HOLD']
  if (benchIntent === 'SOLO' && decision !== 'SOLO' && decision !== 'HOLD') {
    return [`Bench intent is SOLO — Lead may SOLO or HOLD, not ${decision}`]
  }
  if (benchIntent === 'SMITH' && decision === 'SPLIT') {
    return ['Bench intent is SMITH — Lead may not escalate to SPLIT']
  }
  return []
}
