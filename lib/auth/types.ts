// AUTH-02 canonical application-security type boundary.
// Narrow on purpose: only what authorization needs. No tokens, no connection
// details, no provider credentials.

export type AccountType = 'internal' | 'external'

export type RoleCode =
  | 'owner'
  | 'agent'
  | 'viewer'
  | 'client'

export type AuthorityCode =
  | 'portal.read'
  | 'crm.write'
  | 'listing.write'
  | 'deal.read'
  | 'deal.write'
  | 'settings.read'
  | 'settings.manage'
  | 'tech.access'
  | 'external.properties.save'
  | 'external.deal.read_own'

// The effective application actor after authentication + mapping + role
// projection. This is the only shape business services should depend on.
export type ActingUser = {
  appUserId: string
  displayName: string
  email: string | null
  accountType: AccountType
  roleCodes: string[]
  authorityCodes: string[]
  personId: string | null
}

// A provider-verified identity claim, extracted from the session by an adapter.
export type AuthenticatedIdentity = {
  provider: string
  providerSubject: string
  providerEmail: string | null
}

// Serialized actor projection for client-side UI gating only (hiding buttons /
// nav items). Cosmetic — never the security boundary. The authoritative checks
// are server-side (requirePortalAccess / getActingUser + requireAuthority).
export type PortalActorSnapshot = {
  displayName: string
  accountType: AccountType
  authorityCodes: string[]
}
