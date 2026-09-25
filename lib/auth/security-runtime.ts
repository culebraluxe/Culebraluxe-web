import type { ActingUser, SecurityLevel } from './types'
import {
  rustApiAuthorize,
  rustApiResolveIdentity,
  type RustAuthorizationDecision,
  type RustIdentityResolution,
} from '@/lib/rust-api/client'

type TestIdentityResolution =
  | { kind: 'known'; actingUser: ActingUser; securityLevel?: SecurityLevel }
  | { kind: 'unmapped' }
  | { kind: 'inactive' }

type SecurityRepositorySeam = {
  resolveProviderSubject(
    provider: string,
    providerSubject: string,
  ): Promise<TestIdentityResolution>
}

type AuthorizationPort = {
  authorize(input?: unknown): Promise<RustAuthorizationDecision>
}

let securityRepositoryForTesting: SecurityRepositorySeam | null = null
let authorizationPortForTesting: AuthorizationPort | null = null

/** Test-only identity seam. Production always resolves through Rust. */
export function setSecurityRepositoryForTesting(
  repository: SecurityRepositorySeam | null,
): void {
  securityRepositoryForTesting = repository
}

/** Test-only authorization seam. Production always decides through Rust. */
export function setAuthorizationPortForTesting(
  port: AuthorizationPort | null,
): void {
  authorizationPortForTesting = port
}

/**
 * Auth.js proves the provider identity; Rust owns provider-subject -> app user.
 * No TypeScript service kernel, repository, role hierarchy, or entitlement
 * interpretation participates in production.
 */
export async function resolveApplicationSecurityIdentity(
  provider: string,
  providerSubject: string,
): Promise<RustIdentityResolution> {
  if (securityRepositoryForTesting) {
    try {
      const resolution =
        await securityRepositoryForTesting.resolveProviderSubject(
          provider,
          providerSubject,
        )
      if (resolution.kind !== 'known') return resolution
      return {
        kind: 'known',
        actingUser: resolution.actingUser,
        securityLevel:
          resolution.securityLevel ??
          resolution.actingUser.securityLevel ??
          'GUEST',
      }
    } catch {
      return { kind: 'unmapped' }
    }
  }

  try {
    return await rustApiResolveIdentity(provider, providerSubject)
  } catch {
    return { kind: 'unmapped' }
  }
}

/**
 * Ask Rust one question: may the signed-in principal perform this named action?
 * Rust owns the action catalog, operation kind, Casbin policy, role hierarchy,
 * entitlements, and final decision.
 */
export async function authorizeApplicationAction(
  action: string,
): Promise<RustAuthorizationDecision> {
  if (authorizationPortForTesting) {
    return authorizationPortForTesting.authorize({ action })
  }
  return rustApiAuthorize(action)
}
