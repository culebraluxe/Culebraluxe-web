// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: public surface.
//
// The adapter module is self-contained behind the DOC-03 SignatureProvider
// seam: import the provider factory and configure the application-wide
// registry (lib/signature/provider-registry.ts) at app startup — the router
// dispatches by the configured provider, never by provider-specific command.
// ---------------------------------------------------------------------------

export {
  BOLD_SIGN_CONFIG_KEYS,
  BOLD_SIGN_DEFAULTS,
  loadBoldSignConfig,
  type BoldSignConfig,
} from './config'
export {
  BoldSignProviderError,
  classifyBoldSignError,
  isTransientHttpStatus,
  type BoldSignErrorClassification,
} from './errors'
export { BoldSignClient, type BoldSignClientDeps } from './client'
export { BoldSignSignatureProvider, type BoldSignSignatureProviderDeps } from './adapter'
export {
  BOLD_SIGN_SIGNATURE_HEADER,
  parseBoldSignWebhookPayload,
  signBoldSignWebhook,
  verifyBoldSignWebhookSignature,
  type BoldSignWebhookEvent,
} from './webhook'
export { mapBoldSignWebhookEvent } from './events'

import type { BoldSignSignatureProviderDeps } from './adapter'
import { BoldSignSignatureProvider } from './adapter'
import { loadBoldSignConfig } from './config'

/**
 * Build the BoldSign provider from the environment (fail closed on missing
 * credentials). `deps` may override the client / store / clock for tests.
 */
export function createBoldSignProvider(
  env: NodeJS.ProcessEnv = process.env,
  deps: Omit<BoldSignSignatureProviderDeps, 'config'> = {},
): BoldSignSignatureProvider {
  return new BoldSignSignatureProvider({ config: loadBoldSignConfig(env), ...deps })
}
