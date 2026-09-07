import type {
  AuthorizationDecision,
  AuthorizationPort,
  AuthorizationRequest,
} from '../core'
import type { SecurityLevel } from '../security/level'

/**
 * ENTITLEMENT stub with one enforced rule.
 *
 * Open-stub defaults (temporary, visible in every decision):
 *   - GUEST (or a missing principal treated as GUEST) may run queries, never
 *     commands.
 *   - every non-GUEST principal may run queries and commands.
 * One real rule now: `contract.execute` is high-value and near-irreversible, so
 * it requires BUSINESS_POWER_USER or ROOT regardless of the open-stub defaults.
 *
 * Each decision stamps its `policyId` + `mode`, and BaseService records it on
 * the audit event, so "authorization ran" is always observable.
 */
export class EntitlementService implements AuthorizationPort {
  readonly mode = 'open-stub' as const

  async authorize(request: AuthorizationRequest): Promise<AuthorizationDecision> {
    const level: SecurityLevel = request.principal?.level ?? 'GUEST'
    const isGuest = level === 'GUEST'

    if (request.domain === 'contract' && request.operation === 'contract.execute') {
      if (level === 'BUSINESS_POWER_USER' || level === 'ROOT') {
        return {
          allowed: true,
          reason: 'contract.execute allowed for an elevated principal',
          policyId: 'rule:contract.execute.level',
          mode: 'enforced',
        }
      }
      return {
        allowed: false,
        reason: 'contract.execute requires BUSINESS_POWER_USER or ROOT',
        policyId: 'rule:contract.execute.level',
        mode: 'enforced',
      }
    }

    if (request.kind === 'query') {
      return {
        allowed: true,
        reason: isGuest ? 'open-stub: GUEST query allowed' : 'open-stub: query allowed',
        policyId: 'stub:open',
        mode: 'open-stub',
      }
    }

    if (isGuest) {
      return {
        allowed: false,
        reason: 'open-stub: GUEST (or missing principal) cannot run commands',
        policyId: 'stub:guest-command-deny',
        mode: 'open-stub',
      }
    }

    return {
      allowed: true,
      reason: `open-stub: ${level} command allowed`,
      policyId: 'stub:open',
      mode: 'open-stub',
    }
  }
}
