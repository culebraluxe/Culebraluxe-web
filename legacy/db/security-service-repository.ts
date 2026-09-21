import {
  resolveProviderSubject,
  type IdentityResolution,
} from '@/legacy/db/auth-identity'
import { getSecurityPrincipal } from '@/legacy/db/auth-user'
import type {
  SecurityRepository,
  SecurityRepositoryIdentityResolution,
} from '@/legacy/services/security'

/**
 * Security repository adapter over the proven AUTH-02 projection.
 *
 * No new security tables are introduced here:
 * Google/Auth.js subject -> auth_identity -> app_user -> security_role.
 */
export class SqlSecurityRepository implements SecurityRepository {
  async resolveProviderSubject(
    provider: string,
    providerSubject: string,
  ): Promise<SecurityRepositoryIdentityResolution> {
    const resolution: IdentityResolution = await resolveProviderSubject(
      provider,
      providerSubject,
    )
    return resolution
  }

  async getPrincipal(appUserId: string) {
    return getSecurityPrincipal(appUserId)
  }
}
