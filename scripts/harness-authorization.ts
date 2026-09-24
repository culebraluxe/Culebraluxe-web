/**
 * A decision double for HARNESSES AND SCRIPTS, which exercise a service rather than authorization.
 *
 * They used to pass `StaticAuthorizationPolicyProvider` to mean "do not think about authorization here". That class is
 * gone — the port asks Rust now — and the Rust client is `server-only`, so such a caller could not load it outside a
 * Next build even if it wanted to. This keeps those callers exactly as auth-free as they were, and states so out loud
 * instead of appearing to test something it does not.
 *
 * NOT FOR PRODUCTION PATHS. Anything serving a request should use `new AuthorizationService()` and get the real answer.
 */
export const harnessAuthorization = async () => ({
  allowed: true,
  reason: 'harness: authorization not under test',
  policyId: 'harness:permit',
  mode: 'enforced' as const,
})
