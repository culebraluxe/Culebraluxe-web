// AUTH-04 non-secret environment & secret readiness probe.
//
// Read-only. Returns booleans ONLY — never values, never URLs, never tokens,
// never hashes. It extends the AUTH-02 break-glass readiness pattern
// (configured / enabled booleans) to the whole production-secret surface so an
// operator can see, without leaking anything, whether each credential the
// application depends on is configured for the current environment.
//
// The probe is deliberately pure: it never imports the database client, so it
// renders and reports even when the database is misconfigured (db/client.ts
// throws at import time when the URL for the current APP_ENV is missing).
//
// Fail-closed contract (see docs/environment-audit.md):
//   - production requires DATABASE_URL_PROD, AUTH_SECRET, the Google OAuth
//     client pair (AUTH_GOOGLE_ID / AUTH_GOOGLE_SECRET), GOOGLE_MAPS_API_KEY,
//     and the Mux production token pair;
//   - absence reports `false` ("not configured") here, and production code
//     paths fail loudly (db/client.ts throws) or degrade to "not configured"
//     (the public property page renders without a map) instead of silently
//     falling back to a DEV/demo credential;
//   - GOOGLE_MAPS_DEMO_KEY present in a production process is flagged as a
//     misconfiguration (googleMapsDemoKeyAbsentInProduction === false).

import { getAuthProviderConfig } from "./auth/provider-config"
import { getBreakGlassConfig } from "./auth/break-glass-config"

export type EnvironmentReadiness = {
  // Environment identity (non-secret).
  isProduction: boolean
  // Database.
  databaseConfigured: boolean
  databaseDevProdSeparated: boolean
  // Auth.js.
  authSecretConfigured: boolean
  authProviderConfigured: boolean
  // Break-glass (AUTH-02 pattern reuse).
  breakGlassConfigured: boolean
  breakGlassEnabled: boolean
  // Google Maps.
  googleMapsKeyConfigured: boolean
  googleMapsDemoKeyAbsentInProduction: boolean
  // Mux.
  muxConfigured: boolean
  // Aggregate fail-closed posture for the current environment.
  allProductionRequiredConfigured: boolean
}

function configured(value: string | null | undefined): boolean {
  return (value?.trim() || null) !== null
}

export function getEnvironmentReadiness(): EnvironmentReadiness {
  const isProduction = process.env.NODE_ENV === "production"

  const databaseUrlProd = process.env.DATABASE_URL_PROD?.trim() || null
  const databaseUrlDev = process.env.DATABASE_URL_DEV?.trim() || null
  const databaseConfigured = isProduction
    ? databaseUrlProd !== null
    : databaseUrlDev !== null
  // DEV and PROD Neon branches stay separate: both URLs present and different.
  const databaseDevProdSeparated =
    databaseUrlProd !== null &&
    databaseUrlDev !== null &&
    databaseUrlProd !== databaseUrlDev

  const authProvider = getAuthProviderConfig()
  const authProviderConfigured =
    authProvider.clientId !== null && authProvider.clientSecret !== null
  const authSecretConfigured = configured(process.env.AUTH_SECRET)

  const breakGlass = getBreakGlassConfig()
  const breakGlassConfigured = Boolean(
    breakGlass.appUserId && breakGlass.secretHash,
  )

  const googleMapsProdKey = process.env.GOOGLE_MAPS_API_KEY?.trim() || null
  const googleMapsDemoKey = process.env.GOOGLE_MAPS_DEMO_KEY?.trim() || null
  // Mirrors the selection in app/properties/[slug]/page.tsx: production reads
  // only GOOGLE_MAPS_API_KEY; non-production prefers GOOGLE_MAPS_DEMO_KEY.
  const googleMapsKeyConfigured = isProduction
    ? googleMapsProdKey !== null
    : googleMapsDemoKey !== null || googleMapsProdKey !== null
  // Fail-closed: the demo key must never be present in a production process.
  const googleMapsDemoKeyAbsentInProduction =
    !isProduction || googleMapsDemoKey === null

  const muxConfigured = isProduction
    ? configured(process.env.MUX_TOKEN_ID_PROD) &&
      configured(process.env.MUX_TOKEN_SECRET_PROD)
    : configured(process.env.MUX_TOKEN_ID_DEV) &&
      configured(process.env.MUX_TOKEN_SECRET_DEV)

  const productionRequired = [
    databaseConfigured,
    authSecretConfigured,
    authProviderConfigured,
    googleMapsKeyConfigured,
    googleMapsDemoKeyAbsentInProduction,
    muxConfigured,
  ]
  const allProductionRequiredConfigured = isProduction
    ? productionRequired.every(Boolean)
    : databaseConfigured

  return {
    isProduction,
    databaseConfigured,
    databaseDevProdSeparated,
    authSecretConfigured,
    authProviderConfigured,
    breakGlassConfigured,
    breakGlassEnabled: breakGlass.enabled,
    googleMapsKeyConfigured,
    googleMapsDemoKeyAbsentInProduction,
    muxConfigured,
    allProductionRequiredConfigured,
  }
}
