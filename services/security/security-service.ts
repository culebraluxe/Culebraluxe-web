import {
  BaseService,
  type ServiceInfrastructure,
  type ServiceOperationDefinitions,
} from '../core'
import type { ActingUser } from '@/lib/auth/types'
import type { SecurityRepository } from './repository'
import { resolveSecurityLevel } from './level'
import {
  SECURITY_OPERATIONS,
  type SecurityOperationMap,
  type SecurityPrincipal,
} from './types'

function principalFor(actingUser: ActingUser | null): SecurityPrincipal | null {
  if (!actingUser) return null
  return {
    actingUser,
    level: resolveSecurityLevel(actingUser.roleCodes),
  }
}

/**
 * Canonical Security service.
 *
 * Authentication remains provider-facing (Auth.js). Security owns the
 * application identity projection and four broad access levels. Fine-grained
 * operation/resource entitlements are intentionally a separate service seam.
 */
export class SecurityService extends BaseService<SecurityOperationMap> {
  readonly domain = 'security'
  readonly version = '1'
  readonly description =
    'Owns application identity resolution and broad Portal security levels.'
  protected readonly operations: ServiceOperationDefinitions<SecurityOperationMap>

  constructor(
    private readonly repository: SecurityRepository,
    infrastructure: ServiceInfrastructure = {},
  ) {
    super(infrastructure)

    this.operations = {
      [SECURITY_OPERATIONS.RESOLVE_IDENTITY]: {
        kind: 'query',
        description:
          'Resolve a provider-authenticated subject to the canonical application security principal.',
        authorization: 'security.identity.resolve',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) => {
          const resolution = await this.repository.resolveProviderSubject(
            request.provider,
            request.providerSubject,
          )
          if (resolution.kind !== 'known') return resolution

          return {
            kind: 'known' as const,
            principal: {
              actingUser: resolution.actingUser,
              level: resolveSecurityLevel(resolution.actingUser.roleCodes),
            },
          }
        },
      },
      [SECURITY_OPERATIONS.GET_PRINCIPAL]: {
        kind: 'query',
        description:
          'Return the canonical application security principal for an active app_user.',
        authorization: 'security.principal.read',
        idempotent: true,
        execution: { mode: 'inline' },
        handle: async (request) =>
          principalFor(await this.repository.getPrincipal(request.appUserId)),
      },
    }
  }

  invariants() {
    return [
      'Auth provider claims never grant Portal access without an exact auth_identity mapping.',
      'security_role is the source of broad application access level truth.',
      'Unknown or empty security roles resolve to GUEST, never to elevated access.',
      'Fine-grained entitlements are not owned by SecurityService.',
    ] as const
  }
}
