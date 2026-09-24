import type {
  AuthorizationDecision,
  AuthorizationPort,
  AuthorizationRequest,
  ServiceOperationKind,
} from '@/legacy/services/core'
import { hasSecurityLevel, type SecurityLevel } from '@/legacy/services/security/level'
// NOTE: no static import of the Rust client here — it is `server-only` and is loaded lazily in askRustToDecide,
// so harnesses and scripts that import this module without ever authorizing stay loadable.

/** Where a decision comes from. Production passes nothing and gets the Rust security service. */
export type DecisionDelegate = (
  request: AuthorizationRequest,
) => Promise<AuthorizationDecision>

/**
 * THE POLICY-AS-DATA TYPES BELOW ARE HISTORY, kept only because the field names are still read by older callers.
 *
 * The rules they describe are no longer evaluated here: `AuthorizationService` asks the Rust security service, which
 * owns the ROOT-only rule for the `security.*.manage` actions, the entitlement grants and the level hierarchy. What
 * remains in this file is the port and its delegate.
 */

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

/**
 * THE AUTHORIZATION PORT, decided by Rust.
 *
 * There is ONE engine for "may this principal do this action", and it is the Rust security service: the Casbin port,
 * the ROOT-only rule for `security.entitlement.manage` / `security.role.manage`, the entitlement grants and the level
 * hierarchy. This class is the boundary the TypeScript kernel talks to; it holds no rules of its own.
 *
 * WHY IT USED TO HOLD RULES, AND WHAT THAT COST. It carried a policy table read from the database plus its own
 * role-entitlement query, so TypeScript could decide without asking anyone. Two engines meant two answers: the
 * ROOT-only rule existed in Rust and NOT here, so a grant row Rust would refuse to honour would have been honoured on
 * this side. They also disagreed about freshness — a revocation bit on the next Rust request, and whenever the
 * TypeScript query happened to run.
 *
 * A call that cannot reach the authority is NOT an allow: the client throws, BaseService fails the operation, and
 * "we could not check" stays distinct from "it is fine".
 */
export class AuthorizationService implements AuthorizationPort {
  readonly mode = 'enforced' as const

  constructor(private readonly decide: DecisionDelegate = askRustToDecide) {}

  async authorize(request: AuthorizationRequest): Promise<AuthorizationDecision> {
    return this.decide(request)
  }
}

/**
 * The production delegate: one question to the service that owns the answer.
 *
 * THE REQUEST'S DOMAIN AND OPERATION ARE NOT SENT. Rust derives them from the action, because its policy keys rules on
 * those fields — a client able to relabel its own domain could dodge the `contract.execute` level floor. The action
 * and its kind are the whole question, and the catalog refuses any action it does not know.
 *
 * The client is imported LAZILY because it is `server-only` (it reads the Auth.js session and signs bridge headers),
 * and this module is imported by scripts and harnesses that run outside a Next build and never authorize. Loading it
 * on the path that needs it keeps those callers loadable instead of failing at import time.
 */
async function askRustToDecide(request: AuthorizationRequest): Promise<AuthorizationDecision> {
  const { rustApiAuthorize } = await import('@/lib/rust-api/client')
  const decision = await rustApiAuthorize(request.action, request.kind)
  return {
    allowed: decision.allowed,
    reason: decision.reason,
    policyId: decision.policyId,
    mode: 'enforced',
  }
}
