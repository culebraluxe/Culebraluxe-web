import type { ActingUser } from '@/lib/auth/types'

export type SecurityRepositoryIdentityResolution =
  | { kind: 'known'; actingUser: ActingUser }
  | { kind: 'unmapped' }
  | { kind: 'inactive' }

/**
 * Persistence boundary for Security. The first adapter deliberately reuses the
 * existing AUTH-02 auth_identity/app_user/security_role projection.
 */
export interface SecurityRepository {
  resolveProviderSubject(
    provider: string,
    providerSubject: string,
  ): Promise<SecurityRepositoryIdentityResolution>
  getPrincipal(appUserId: string): Promise<ActingUser | null>
}
