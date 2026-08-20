// AUTH-02 break-glass root configuration.
//
// All values come from server-side environment configuration. Nothing here is
// committed, and the secret is only ever a scrypt hash.

export type BreakGlassConfig = {
  enabled: boolean
  appUserId: string | null
  secretHash: string | null
}

export function getBreakGlassConfig(): BreakGlassConfig {
  const enabled = process.env.AUTH_BREAK_GLASS_ENABLED === 'true'
  const appUserId = process.env.AUTH_BREAK_GLASS_APP_USER_ID?.trim() || null
  const secretHash = process.env.AUTH_BREAK_GLASS_SECRET_HASH?.trim() || null

  return { enabled, appUserId, secretHash }
}
