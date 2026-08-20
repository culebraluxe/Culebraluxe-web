// AUTH-02 provider configuration boundary. No provider-specific code leaks
// beyond this module and the adapter layer.

export type AuthProviderConfig = {
  provider: string
  clientId: string | null
  clientSecret: string | null
  issuer: string | null
}

export function getAuthProviderConfig(): AuthProviderConfig {
  return {
    provider: process.env.AUTH_PROVIDER?.trim() || 'google',
    clientId: process.env.AUTH_GOOGLE_ID?.trim() || null,
    clientSecret: process.env.AUTH_GOOGLE_SECRET?.trim() || null,
    issuer: process.env.AUTH_ISSUER?.trim() || null,
  }
}
