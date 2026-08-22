// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam: configured-provider dispatch.
//
// "The router dispatches by configured provider, never by provider-specific
// command." This registry holds the ONE configured SignatureProvider for the
// application (the fake in tests/DEV; the BoldSign adapter behind the same
// interface in DOC-04). Command handlers and the application router resolve
// the provider here — there is never a per-provider branch in the command set.
// ---------------------------------------------------------------------------

import type { SignatureProvider } from './provider'

export class SignatureProviderRegistry {
  private provider: SignatureProvider | null = null

  configure(provider: SignatureProvider): void {
    this.provider = provider
  }

  /** The configured provider. Throws when none is configured (fail closed). */
  get(): SignatureProvider {
    if (!this.provider) {
      throw new Error(
        'No SignatureProvider is configured. Configure one via SignatureProviderRegistry.configure().',
      )
    }
    return this.provider
  }

  isConfigured(): boolean {
    return this.provider !== null
  }

  name(): string | null {
    return this.provider?.name ?? null
  }
}

/** Application-wide singleton (tests configure a fake; DOC-04 configures the
 *  BoldSign adapter behind the same seam). */
export const signatureProviderRegistry = new SignatureProviderRegistry()
