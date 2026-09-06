import type {
  AuthorizationPort,
  AuthorizationRequest,
} from '../core'

/**
 * ENTITLEMENT stub.
 *
 * This is intentionally permissive in the first Security cut: the application
 * did not previously enforce fine-grained Core-service entitlements and this
 * story must not silently change business behavior. BaseService is wired to
 * this explicit port now so a later Entitlement service can replace the policy
 * without another service-kernel retrofit.
 */
export class EntitlementService implements AuthorizationPort {
  readonly mode = 'open-stub' as const

  async authorize(_request: AuthorizationRequest): Promise<boolean> {
    return true
  }
}
