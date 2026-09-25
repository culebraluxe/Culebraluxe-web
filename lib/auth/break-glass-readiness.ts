// Break-glass readiness projection.
//
// The UI consumes booleans only. Database/security facts come from the Rust
// SupportDiagnosticsService; this edge never reads security tables directly and
// never returns the configured app-user id, secret hash, OAuth secrets or tokens.

import {
  rustApiBreakGlassReadiness,
  type RustBreakGlassReadiness,
} from '@/lib/rust-api/client'

export type BreakGlassReadiness = RustBreakGlassReadiness

export async function getBreakGlassReadiness(): Promise<BreakGlassReadiness> {
  return rustApiBreakGlassReadiness()
}
