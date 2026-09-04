import { isAssayTerminalRole } from './candidate-assay-handoff'

export type ForgeRecoveryAction = 'retry' | 'hold-human'

export type ForgeRecoveryDecision = {
  action: ForgeRecoveryAction
  humanRequired: boolean
  reason: string
}

/**
 * Forge V6 recovery policy.
 *
 * Runtime recovery is lane-specific policy, not a generic retry loop:
 * - Smith infrastructure interruption may retry while budget remains.
 * - Assay is a human intervention boundary. Any interruption goes to Hold
 *   immediately, regardless of retry budget. It never restarts Smith.
 * - Other roles retain conservative retry-while-budget behavior.
 */
export function decideRuntimeRecovery(input: {
  role: string | null | undefined
  attempts: number
  maxAttempts: number
  reason: string
}): ForgeRecoveryDecision {
  if (isAssayTerminalRole(input.role)) {
    return {
      action: 'hold-human',
      humanRequired: true,
      reason:
        `Assay interrupted: ${input.reason}. Human intervention required; ` +
        'no automatic Assay retry and no Smith restart.',
    }
  }

  if (input.attempts < input.maxAttempts) {
    return {
      action: 'retry',
      humanRequired: false,
      reason: input.reason,
    }
  }

  return {
    action: 'hold-human',
    humanRequired: true,
    reason:
      `runtime retry budget exhausted (${input.attempts}/${input.maxAttempts}): ${input.reason}`,
  }
}
