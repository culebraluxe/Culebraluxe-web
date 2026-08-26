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
import { devAuthLog } from './dev-auth-log'

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

  // AUTH-08G — DEV flight markers around the canonical provider-subject →
  // auth_identity → app_user mapping. Lookup start is logged BEFORE the DB
  // projection; MAPPED is logged only when the subject resolves to a known
  // application actor. Failures are surfaced by the caller's exact safe
  // reason code (requirePortalAccess logs error.code, e.g. unmapped-identity /
  // inactive-account). Authentication and authorization stay separate: Auth.js
  // never touches this DB projection.
  devAuthLog('AUTH_APP_IDENTITY_LOOKUP_STARTED')
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

  devAuthLog('AUTH_APP_IDENTITY_MAPPED')
  return resolution.actingUser
}
