// AUTH-02 narrow adapter contract between a login/session provider and the
// CulebraLuxe application security layer.
//
// Provider responsibility: establish the session and expose the verified
// provider subject (and optional verified email). Provider cookies/tokens stay
// inside the adapter and never leak into business services.
//
// Application security responsibility (downstream of this adapter): map
// provider subject → auth_identity → app_user → roles → authorities.

import type { AuthenticatedIdentity } from './types'

export type SessionAdapter = {
  // Returns the authenticated provider identity, or null when no valid session
  // exists. The provider subject MUST be a stable, provider-verified value.
  getSession(): Promise<AuthenticatedIdentity | null>
}
