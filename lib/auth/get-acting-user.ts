// AUTH-02 canonical application function: getActingUser().
//
// The single place where a session becomes an application actor. Business
// modules must call this (or receive an ActingUser) rather than touching the
// auth library / session directly.

import type { SessionAdapter } from './session-adapter'
import type { ActingUser } from './types'
import { resolveProviderSubject } from '@/db/auth-identity'
import {
  InactiveAccountError,
  UnauthenticatedError,
  UnmappedIdentityError,
} from './errors'
import { isPortalAuthBypass, portalAuthBypassActor } from './dev-bypass'

export async function getActingUser(
  adapter: SessionAdapter,
): Promise<ActingUser> {
  // TEMP STARTUP AUTH BYPASS — same flag as the portal layout guard, so
  // browsing AND server actions (create form, save, issue) work in local dev.
  if (isPortalAuthBypass()) {
    return portalAuthBypassActor()
  }

  const session = await adapter.getSession()
  if (!session) {
    throw new UnauthenticatedError()
  }

  const resolution = await resolveProviderSubject(
    session.provider,
    session.providerSubject,
  )

  if (resolution.kind === 'unmapped') {
    throw new UnmappedIdentityError(session.provider, session.providerSubject)
  }
  if (resolution.kind === 'inactive') {
    throw new InactiveAccountError()
  }

  return resolution.actingUser
}
