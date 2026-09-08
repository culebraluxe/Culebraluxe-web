// -----------------------------------------------------------------------------
// Run guardrails — spend/stop lines injected into EVERY model-backed role's
// prompt (scout, architect, lead, smith), not just Lead.
//
// Cline-vs-OpenCode quality is envelope, not brain. The single biggest missing
// control is a hard stop: a one-shot headless run must know it has a budget,
// must not brute-force, and must STOP to HOLD when a job is ambiguous/oversized.
// -----------------------------------------------------------------------------
import { runWallClockBudgetMs } from './agent-runtime-adapter'
import { renderModelCostLines } from './model-prices'

export function buildRunGuardrailsDirective(env: NodeJS.ProcessEnv = process.env): string {
  const minutes = Math.max(1, Math.round(runWallClockBudgetMs(env) / 60000))
  return [
    ...renderModelCostLines(),
    `Your run has a HARD ${minutes}-minute wall-clock budget; the runner cancels it automatically at that limit.`,
    'Do NOT brute-force or wander. Produce the bounded deliverable for THIS node only, then STOP.',
    'If the work is ambiguous, oversized, or would clearly exceed the budget, do NOT grind. Report the blocker and choose the HOLD/stop disposition so a human decides.',
  ].join('\n')
}
