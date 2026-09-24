import type {
  AuthorizationDecision,
  AuthorizationPort,
  AuthorizationRequest,
  ServiceOperationKind,
} from '@/legacy/services/core'
import { hasSecurityLevel, type SecurityLevel } from '@/legacy/services/security/level'

/**
 * Policy-as-data authorization resolver (first cut).
 *
 * Deliberately small and level-based — NOT a user->role->screen->feature
 * matrix and NOT a ReBAC graph. A rule is: (optional domain/action/operation/
 * kind) + a minimum SecurityLevel. Ordering: the most specific matching rule
 * wins; ROOT is an explicit superuser; a request with no matching rule falls
 * back to kind-based defaults (GUEST may query, never command).
 *
 * Policies come from a simple `AuthorizationPolicyProvider` seam (code constants
 * today; a SQL provider can replace it later without touching the resolver).
 */
export type AuthorizationPolicy = {
  id: string
  description?: string
  domain?: string
  action?: string
  operation?: string
  kind?: ServiceOperationKind
  minLevel: SecurityLevel
}

export interface AuthorizationPolicyProvider {
  policies(): Promise<readonly AuthorizationPolicy[]>
}

/** Exact action grants for a resolved user; the implementation reads the same
 * role_entitlement rows as the Rust Casbin adapter. */
export interface EntitlementGrantProvider {
  hasGrant(appUserId: string, action: string, kind: ServiceOperationKind): Promise<boolean>
}

/** Built-in rules; the enforce list starts where the open-stub left off. */
export const DEFAULT_AUTHORIZATION_POLICIES: readonly AuthorizationPolicy[] = [
  {
    id: 'rule:contract.execute',
    description: 'Executing a Contract is high-value and near-irreversible.',
    domain: 'contract',
    operation: 'contract.execute',
    kind: 'command',
    minLevel: 'BUSINESS_POWER_USER',
  },
]

const ROOT_POLICY_ID = 'authorization:root'

export class AuthorizationService implements AuthorizationPort {
  readonly mode = 'enforced' as const

  constructor(
    private readonly provider: AuthorizationPolicyProvider,
    private readonly grants?: EntitlementGrantProvider,
  ) {}

  async authorize(request: AuthorizationRequest): Promise<AuthorizationDecision> {
    const level: SecurityLevel = request.principal?.level ?? 'GUEST'
    const isGuest = level === 'GUEST'

    // ROOT is the explicit superuser: god access, one place, never per-handler.
    if (level === 'ROOT') {
      return {
        allowed: true,
        reason: 'ROOT superuser',
        policyId: ROOT_POLICY_ID,
        mode: 'enforced',
      }
    }

    const rules = await this.provider.policies()
    const rule = this.match(request, rules)
    // The rule is a minimum-level floor; a role grant is also required. This
    // keeps the existing high-value rules while making per-action grants real.
    const levelAllowed = !rule || hasSecurityLevel(level, rule.minLevel)
    const grantAllowed = this.grants && request.principal
      ? await this.grants.hasGrant(request.principal.appUserId, request.action, request.kind)
      : undefined
    if (rule) {
      const allowed = levelAllowed && (grantAllowed ?? true)
      return {
        allowed,
        reason: !levelAllowed
          ? `${rule.id}: ${level} is below required ${rule.minLevel}`
          : grantAllowed === false
            ? `${rule.id}: no matching role entitlement`
            : `${rule.id}: ${level} meets required ${rule.minLevel}`,
        policyId: rule.id,
        mode: 'enforced',
      }
    }

    if (grantAllowed !== undefined) {
      return {
        allowed: grantAllowed,
        reason: grantAllowed ? 'role entitlement granted' : 'no matching role entitlement',
        policyId: `entitlement:${request.action}`,
        mode: 'enforced',
      }
    }

    // No rule: kind-based default. GUEST (unauthenticated) may read, never write.
    if (request.kind === 'query') {
      return {
        allowed: true,
        reason: isGuest ? 'default: GUEST query allowed' : 'default: query allowed',
        policyId: 'default:query',
        mode: 'enforced',
      }
    }
    if (isGuest) {
      return {
        allowed: false,
        reason: 'default: GUEST (or missing principal) cannot run commands',
        policyId: 'default:guest.command-deny',
        mode: 'enforced',
      }
    }
    return {
      allowed: true,
      reason: `default: ${level} command allowed`,
      policyId: 'default:command',
      mode: 'enforced',
    }
  }

  /** Highest-specificity matching rule wins; ties keep first definition order. */
  private match(
    request: AuthorizationRequest,
    policies: readonly AuthorizationPolicy[],
  ): AuthorizationPolicy | null {
    let best: AuthorizationPolicy | null = null
    let bestSpecificity = -1
    for (const policy of policies) {
      if (policy.domain !== undefined && policy.domain !== request.domain) continue
      if (policy.operation !== undefined && policy.operation !== request.operation) continue
      if (policy.action !== undefined && policy.action !== request.action) continue
      if (policy.kind !== undefined && policy.kind !== request.kind) continue
      const specificity =
        (policy.domain === undefined ? 0 : 1) +
        (policy.action === undefined ? 0 : 1) +
        (policy.operation === undefined ? 0 : 1) +
        (policy.kind === undefined ? 0 : 1)
      if (specificity > bestSpecificity) {
        best = policy
        bestSpecificity = specificity
      }
    }
    return best
  }
}

/** Constant provider wrapping a static rule set (the first-cut policy source). */
export class StaticAuthorizationPolicyProvider implements AuthorizationPolicyProvider {
  constructor(private readonly rules: readonly AuthorizationPolicy[] = DEFAULT_AUTHORIZATION_POLICIES) {}
  async policies(): Promise<readonly AuthorizationPolicy[]> {
    return this.rules
  }
}
