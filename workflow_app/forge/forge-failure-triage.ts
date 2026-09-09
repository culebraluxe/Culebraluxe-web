// ---------------------------------------------------------------------------
// forge-failure-triage — chunk-failure classification + bounded retry policy.
//
// Forge-native TS port of claude-orchestrate's failure taxonomy + hard retry
// budget (MIT; portable/orchestrator.md), adapted to Forge's chunk model. When a
// Smith chunk fails, triage it ONCE into a class and take the single bounded
// action. Never enter an escalation ladder. A surfaced unit parks only itself and
// its dependents; independent passed units still ship.
//
// Provenance: taxonomy + budget translated from claude-orchestrate (MIT,
// github.com/midego1/claude-orchestrate). Reimplemented here; no runtime import.
// ---------------------------------------------------------------------------

import { MAX_MODEL_ESCALATION, MAX_REPAIR_PER_CHUNK } from './forge-dispatchability'

export type ChunkFailureClass =
  | 'SPEC'
  | 'ENV'
  | 'CAPABILITY'
  | 'VERIFICATION_GAP'
  | 'SCOPE_EXPANSION'

export type TriageAction =
  | { kind: 'rewrite-spec-retry'; detail: string }
  | { kind: 'fix-env-retry'; detail: string }
  | { kind: 'escalate-model'; detail: string }
  | { kind: 'split-remaining'; detail: string }
  | { kind: 'hold'; detail: string }

/**
 * Map a classified chunk failure to its ONE bounded action:
 *   SPEC            -> Lead repairs the contract; retry the SAME Smith (escalating
 *                      a bad spec buys an expensive wrong answer).
 *   ENV             -> fix the environment (flaky test / branch / dep / timeout);
 *                      retry the SAME Smith. Unclear cases default here.
 *   CAPABILITY      -> contract sound + env clean but the model can't do it:
 *                      escalate one model step, OR split the remaining work.
 *   SCOPE_EXPANSION -> material scope growth => HOLD (new story / back to Lead).
 *   VERIFICATION_GAP-> the repo's test infra cannot exercise it: do NOT escalate
 *                      (a stronger model buys another unverifiable attempt).
 *                      Reduce to the verifiable subset and HOLD/surface the rest.
 */
export function triageActionFor(failureClass: ChunkFailureClass): TriageAction {
  switch (failureClass) {
    case 'SPEC':
      return {
        kind: 'rewrite-spec-retry',
        detail: 'spec failure: rewrite the contract, retry the SAME Smith/tier',
      }
    case 'ENV':
      return {
        kind: 'fix-env-retry',
        detail: 'environment failure: fix the env, retry the SAME Smith/tier',
      }
    case 'CAPABILITY':
      return {
        kind: 'escalate-model',
        detail: `capability failure: escalate one model step (max ${MAX_MODEL_ESCALATION} escalation) or split the remaining work`,
      }
    case 'VERIFICATION_GAP':
      return {
        kind: 'hold',
        detail: 'verification gap: reduce to the verifiable subset and surface the rest; do NOT escalate',
      }
    case 'SCOPE_EXPANSION':
      return {
        kind: 'hold',
        detail: 'scope expansion: this is a new story boundary; HOLD and return to Lead',
      }
  }
}

/** Hard budget per chunk (ported): original + MAX_REPAIR_PER_CHUNK same-tier
 * repair + MAX_MODEL_ESCALATION escalation = 3 dispatches. Never an escalation
 * ladder; after the budget, surface the unit with its archived failure history. */
export function chunkBudgetTotal(): number {
  return 1 + MAX_REPAIR_PER_CHUNK + MAX_MODEL_ESCALATION
}

export function chunkBudgetExhausted(
  attemptsUsed: number,
  repairsUsed: number,
  escalationsUsed: number,
): { exhausted: boolean; remainingDispatches: number; canRepair: boolean; canEscalate: boolean } {
  const remainingRepairs = Math.max(0, MAX_REPAIR_PER_CHUNK - repairsUsed)
  const remainingEscalations = Math.max(0, MAX_MODEL_ESCALATION - escalationsUsed)
  const remainingDispatches = Math.max(0, chunkBudgetTotal() - attemptsUsed)
  return {
    exhausted: remainingDispatches <= 0,
    remainingDispatches,
    canRepair: remainingRepairs > 0,
    canEscalate: remainingEscalations > 0,
  }
}

/** Signals for classifying a chunk failure (the "how to tell" heuristic, ported):
 *  reread the dispatch first. If a competent human would need a clarifying
 *  question => SPEC. If the same check fails WITHOUT the worker's change => ENV.
 *  Only with an unambiguous spec and a clean environment is it CAPABILITY.
 *  Unclear cases default to ENV (environment retries are cheapest). */
export type ChunkFailureSignals = {
  rereadAmbiguous: boolean
  checkFailsWithoutChange: boolean
  scopeExpanded: boolean
  structurallyUnverifiable: boolean
}

export function classifyChunkFailure(signals: ChunkFailureSignals): ChunkFailureClass {
  if (signals.scopeExpanded) return 'SCOPE_EXPANSION'
  if (signals.structurallyUnverifiable) return 'VERIFICATION_GAP'
  if (signals.rereadAmbiguous) return 'SPEC'
  if (signals.checkFailsWithoutChange) return 'ENV'
  // Default: an unambiguous spec in a clean environment whose worker can't do it.
  return 'CAPABILITY'
}

/** Evidence-gated discipline (ported): a PASS without cited evidence is a FAIL.
 * Never trust a worker's "done". Use when grading any chunk gate result. */
export function evidenceGatedPass(passed: boolean, evidence: string | null | undefined): boolean {
  if (!passed) return false
  return Boolean(evidence && String(evidence).trim().length > 0)
}


// ---------------------------------------------------------------------------
// Two retry shapes (claude-orchestrate, CHANGELOG + SKILL.md). There is NOT one
// kind of retry:
//   (a) attempt failure with no verified progress -> RESET to the chunk baseline,
//       fresh attempt (the default; no contamination between attempts).
//   (b) a verifier-found gap in otherwise-PASSED work -> INCREMENTAL repair on the
//       SAME branch, ON TOP of the passing commits, then a scoped re-verify that
//       names ONLY the open items (pinned to lastPassedSha..HEAD). Resetting
//       verified work buys no integrity.
// Both shapes count against the SAME 3-dispatch budget. In Forge, either shape
// preserves the resident Smith session/understanding; only CODE state resets.
// ---------------------------------------------------------------------------

export type RetryShape = 'reset-to-baseline' | 'incremental-repair'

/** Choose the retry shape for a failed unit. A specific gap in otherwise-passing
 *  work is an incremental repair on top of the passing commits; anything else
 *  (attempt produced no verified progress) resets to baseline for a clean retry. */
export function retryShapeFor(
  failureClass: ChunkFailureClass,
  partialProgressVerified: boolean,
): RetryShape {
  // Only a concrete verifier-found gap on top of otherwise-PASSED work repairs
  // incrementally. A CAPABILITY/SPEC/ENV attempt that never verified progress
  // resets to baseline. A SCOPE_EXPANSION / VERIFICATION_GAP never retries (hold).
  if (partialProgressVerified && failureClass === 'ENV') return 'incremental-repair'
  return 'reset-to-baseline'
}

/** Directive for the scoped re-verify after an incremental repair: name only the
 *  open items and pin the diff to lastPassedSha..HEAD; do NOT re-litigate PASSed
 *  items. */
export function scopedReverifyDirective(lastPassedSha: string, openItems: string[]): string {
  const items = openItems.length > 0 ? openItems.join('; ') : '(open items)'
  return `Scoped re-verify ONLY the open items: ${items}. Diff is pinned to ${lastPassedSha}..HEAD. Do NOT re-litigate already-PASSed items. A PASS requires cited evidence.`
}

