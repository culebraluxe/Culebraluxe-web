import 'server-only'

import { randomUUID } from 'node:crypto'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { bypassBridgeIdentity } from '@/lib/rust-api/dev-identity'
import {
  buildRustBridgeHeaders,
  buildRustPublicBridgeHeaders,
  resolveInternalApiKey,
  resolveRustApiBaseUrl,
} from '@/lib/rust-api/contract'

export type RustApiSuccess<T> = {
  ok: true
  value: T
  correlationId: string
}

export type RustApiFailure = {
  ok: false
  error: {
    code: string
    message: string
    retryable: boolean
    incidentId?: string | null
  }
  correlationId?: string | null
}

export class RustApiError extends Error {
  readonly status: number
  readonly code: string
  readonly retryable: boolean
  readonly correlationId: string | null
  readonly incidentId: string | null

  constructor(input: {
    status: number
    code: string
    message: string
    retryable?: boolean
    correlationId?: string | null
    incidentId?: string | null
  }) {
    super(input.message)
    this.name = 'RustApiError'
    this.status = input.status
    this.code = input.code
    this.retryable = input.retryable ?? false
    this.correlationId = input.correlationId ?? null
    this.incidentId = input.incidentId ?? null
  }
}

type RustApiReadOptions = {
  correlationId?: string
  causationId?: string
  /**
   * The identity to assert, when the caller already holds one and is NOT asking about its own session.
   *
   * The login seam is the case this exists for: Auth.js has proved a Google subject and the session may not
   * exist yet, so "read as whoever is signed in" is the wrong question. Omitted everywhere else, where the
   * session identity is the right answer by definition.
   */
  identity?: { provider: string; providerSubject: string }
}

/**
 * Resolve an ASSERTED provider identity — the login seam's question ("does this Google subject map to an
 * application user?") as opposed to whoami's ("who am I?").
 *
 * THE THREE ANSWERS ARE THE POINT. "unmapped" and "inactive" are different things to say to a person at the login
 * page, so the security service returns them as data instead of collapsing both into one error. Resolution lives
 * in ONE place — Rust's SecurityService — and this is the transport to it; no caller re-derives it from tables.
 */
export type RustIdentityResolution =
  | {
      kind: 'known'
      actingUser: {
        appUserId: string
        displayName: string
        email: string | null
        accountType: string
        roleCodes: string[]
        authorityCodes: string[]
        entitlementCodes: string[]
        personId: string | null
      }
      securityLevel: string
    }
  | { kind: 'unmapped' }
  | { kind: 'inactive' }

export async function rustApiResolveIdentity(
  provider: string,
  providerSubject: string,
): Promise<RustIdentityResolution> {
  const result = await rustApiRead<RustIdentityResolution>('/v1/security/identity', {
    identity: { provider, providerSubject },
  })
  return result.value
}

export type RustAuthorizationDecision = {
  allowed: boolean
  reason: string
  policyId: string
  mode: string
}

export type RustBreakGlassReadiness = {
  configured: boolean
  enabled: boolean
  rootResolvable: boolean
  rootActive: boolean
  ownerRolePresent: boolean
  auditTableAvailable: boolean
}

export async function rustApiBreakGlassReadiness(): Promise<RustBreakGlassReadiness> {
  const result = await rustApiRead<RustBreakGlassReadiness>(
    '/v1/support/break-glass-readiness',
  )
  return result.value
}

/**
 * Ask the security service to DECIDE one action for the signed-in principal.
 *
 * One brain: the answer comes from the same Casbin port every Rust service uses, so the ROOT-only rule for the
 * `security.*.manage` actions, the entitlement grants and the level hierarchy are applied once instead of being
 * remembered by a second implementation. The question is the action and its kind — nothing else, because the policy
 * keys rules on domain and operation and Rust derives those from the action rather than trusting a client to
 * describe it.
 */
export async function rustApiAuthorize(
  action: string,
  _legacyKind?: 'query' | 'command',
): Promise<RustAuthorizationDecision> {
  const result = await rustApiJsonWrite<RustAuthorizationDecision>(
    '/v1/security/authorize',
    'POST',
    { action },
  )
  return result.value
}

/**
 * The same question, asked by the ANONYMOUS PUBLIC SITE.
 *
 * The public site has no session, so the identified door cannot answer for it — it requires a principal. This one
 * asks the public door, which resolves the caller to the `public-website` system actor in Rust (the same actor the
 * vault's public document route uses) and refuses identity headers. What that actor may reach is the Rust
 * PUBLIC_READ_ACTIONS list: named published reads, queries only. It is NOT "anonymous callers may query" — that rule
 * would have handed the public site `deal.read`.
 */
export async function rustApiAuthorizePublic(
  action: string,
  _legacyKind?: 'query' | 'command',
): Promise<RustAuthorizationDecision> {
  const correlationId = randomUUID()
  const headers = {
    ...buildRustPublicBridgeHeaders({
      internalApiKey: internalApiKey(),
      correlationId,
    }),
    'content-type': 'application/json',
  }

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}/v1/security/authorize/public`, {
      method: 'POST',
      headers,
      body: JSON.stringify({ action }),
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<RustAuthorizationDecision> | RustApiFailure
  try {
    payload = (await response.json()) as
      | RustApiSuccess<RustAuthorizationDecision>
      | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }

  return payload.value
}

export type RustWebsiteLeadNotice = 'sent' | 'already_handled'

/**
 * Ask Rust to email the team and the visitor about ONE website lead that was just saved
 * (`POST /v1/website-intake/{id}/notify`). Only the id is sent: Rust writes the emails from the stored lead and sends
 * them once. Anonymous public door, authorized as `website.lead.notify`.
 */
export async function rustApiNotifyWebsiteLead(submissionId: string): Promise<RustWebsiteLeadNotice> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })

  let response: Response
  try {
    response = await fetch(
      `${rustApiBaseUrl()}/v1/website-intake/${encodeURIComponent(submissionId)}/notify`,
      { method: 'POST', headers, cache: 'no-store' },
    )
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<RustWebsiteLeadNotice> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<RustWebsiteLeadNotice> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }
  return payload.value
}


export async function rustApiTechCockpit(selected?: string | null): Promise<any> {
  const suffix = selected ? '?selected=' + encodeURIComponent(selected) : ''
  const path = `/v1/tech/cockpit${suffix}` as `/v1/${string}`
  return (await rustApiRead<any>(path)).value
}

export async function rustApiTechCommand(body: Record<string, unknown>): Promise<any> {
  return (await rustApiJsonWrite<any>('/v1/tech/cockpit', 'POST', body)).value
}

export type RustPublicListingCopy = { slug: string; tagline: string }

/**
 * The taglines of PUBLISHED listings, from the public door (`GET /v1/public/listing-copy`).
 *
 * Anonymous by construction: the public bridge headers carry no identity, and Rust authorizes the read as the
 * published `property.public.read`. A failure throws a RustApiError like every other call here; the caller decides
 * whether the page can live without the copy.
 */
/**
 * One listing as the Rust public service returns it: the key that opens it, and the facts the grid renders.
 */
/** Shared reader for the public endpoints that answer with a list of listings. A failure is an empty list, not a
 *  broken page: a strip of similar properties is never worth failing a page over. */
async function readListings(response: Response, correlationId: string): Promise<RustPublicListing[]> {
  if (!response.ok) return []
  try {
    const payload = (await response.json()) as RustApiSuccess<RustPublicListing[]> | RustApiFailure
    return payload.ok ? payload.value : []
  } catch {
    return []
  }
}

/**
 * One listing as the Rust public service returns it: the key that opens it, and the facts the grid renders.
 */
export type RustPublicListing = {
  key: string
  id: string
  name: string
  propertyType: string | null
  status: string
  listPrice: number | null
  featured: boolean
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null
  lotSize: number | null
  lotSizeUnits: string | null
  /** The view labels, already spelled: "Ocean", "Sunset". */
  views: string[]
  beachAccess: boolean
  /** The marked hero, or the first photograph. A card always has a picture. */
  heroMediaId: string | null
  heroAlt: string | null
}

/**
 * THE PUBLIC INVENTORY, from the Rust service (`GET /v1/public/listings`).
 *
 * Why this exists: the buyers grid read Postgres directly through the legacy TS db layer, which is the one thing the
 * service kernel says nothing outside a repository should do — "a surface that missed the service-layer refactor".
 * That shortcut is why the public visibility rule had to be maintained in two languages at once. The read lives in
 * Rust now, authorized as the published `property.public.read` and served through the same anonymous public door as
 * the listing copy; this function is the thin bridge, and the caller below it only shapes rows for the screen.
 */
export async function rustApiPublicListings(): Promise<RustPublicListing[]> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}/v1/public/listings`, { headers, cache: 'no-store' })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<RustPublicListing[]> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<RustPublicListing[]> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? false,
      correlationId,
    })
  }

  return payload.value
}

export type RustGuideItem = {
  slug: string
  section: string
  name: string
  eyebrow: string | null
  subtitle: string | null
  area: string | null
  description: string
  note: string | null
  address: string | null
  phone: string | null
  websiteUrl: string | null
  latitude: number | null
  longitude: number | null
  sortOrder: number
  imagePath: string | null
  imageAlt: string | null
}

/** Published Island Guide from the Rust Guide service. */
export async function rustApiPublicGuide(): Promise<RustGuideItem[]> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })
  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}/v1/public/guide`, { headers, cache: 'no-store' })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }
  const payload = (await response.json().catch(() => null)) as RustApiSuccess<RustGuideItem[]> | RustApiFailure | null
  if (!payload) {
    throw new RustApiError({ status: 502, code: 'RUST_API_INVALID_RESPONSE', message: 'Rust API returned a non-JSON response.', retryable: true, correlationId })
  }
  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }
  return payload.value
}

export type RustPublicPropertyMedia = {
  id: string
  role: string
  mediaType: string
  altText: string | null
  caption: string | null
  filename: string | null
  mimeType: string | null
  fileSize: number | null
  sortOrder: number
  /** Present on Mux videos: the site links out to Mux with this. */
  muxPlaybackId: string | null
  aspectRatio: string | null
  durationSeconds: number | null
}

/**
 * One Property from the Rust public service (`GET /v1/public/property?key=…`).
 *
 * `key` is whatever names the row — the slug, the name in any case, the name with spaces, or the id — because the
 * service resolves all of them. `null` means no such listing, which the page turns into a 404.
 */
export type RustPublicProperty = {
  key: string
  name: string
  status: string
  propertyType: string | null
  listPrice: number | null
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null
  lotSize: number | null
  lotSizeUnits: string | null
  yearBuilt: number | null
  architectureNotes: string | null
  shortDescription: string | null
  editorialDescription: string | null
  heroMediaId: string | null
  /** Photographs, Mux videos and documents together — the surface sorts them. */
  media: RustPublicPropertyMedia[]
  videoCount: number
}

/**
 * THE PROPERTY PAGE'S READ, from Rust. The page used to get this through the TypeScript service kernel — the same
 * data, but in the other language, which is how the two drifted apart in the first place.
 */
export async function rustApiPublicProperty(key: string): Promise<RustPublicProperty | null> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })

  let response: Response
  try {
    response = await fetch(
      `${rustApiBaseUrl()}/v1/public/property?key=${encodeURIComponent(key)}`,
      { headers, cache: 'no-store' },
    )
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<RustPublicProperty | null> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<RustPublicProperty | null> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? false,
      correlationId,
    })
  }

  return payload.value
}

/**
 * ONE PHOTOGRAPH'S BYTES, from the public service (`GET /v1/public/media/{id}`).
 *
 * ANSI: this returns raw bytes, not JSON, because that is what a photograph is. `null` means not-found or
 * not-published — the service deliberately does not distinguish, and neither does this.
 */
export async function rustApiPublicMedia(
  id: string,
): Promise<{ contentType: string; bytes: Uint8Array } | null> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}/v1/public/media/${encodeURIComponent(id)}`, {
      headers,
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  if (response.status === 404) return null

  if (!response.ok) {
    throw new RustApiError({
      status: response.status,
      code: 'RUST_API_FAILURE',
      message: 'The public media read failed.',
      retryable: true,
      correlationId,
    })
  }

  return {
    contentType: response.headers.get('content-type') ?? 'application/octet-stream',
    bytes: new Uint8Array(await response.arrayBuffer()),
  }
}

/**
 * LISTINGS LIKE THIS ONE (`GET /v1/public/similar`) — the strip on a property page.
 */
export async function rustApiPublicSimilar(key: string, limit = 3): Promise<RustPublicListing[]> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })
  const response = await fetch(
    `${rustApiBaseUrl()}/v1/public/similar?key=${encodeURIComponent(key)}&limit=${limit}`,
    { headers, cache: 'no-store' },
  )
  return readListings(response, correlationId)
}

/**
 * EVERY SLUG THE SITE CAN SERVE (`GET /v1/public/slugs`) — for the sitemap.
 */
export async function rustApiPublicSlugs(): Promise<string[]> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })
  const response = await fetch(`${rustApiBaseUrl()}/v1/public/slugs`, { headers, cache: 'no-store' })
  if (!response.ok) return []
  const payload = (await response.json()) as RustApiSuccess<string[]> | RustApiFailure
  return payload.ok ? payload.value : []
}

export async function rustApiPublicListingCopy(): Promise<RustPublicListingCopy[]> {
  const correlationId = randomUUID()
  const headers = buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId })

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}/v1/public/listing-copy`, { headers, cache: 'no-store' })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<RustPublicListingCopy[]> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<RustPublicListingCopy[]> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }

  // The body crosses a process boundary: keep only well-formed entries.
  return (Array.isArray(payload.value) ? payload.value : []).filter(
    (entry): entry is RustPublicListingCopy =>
      Boolean(entry) && typeof entry.slug === 'string' && typeof entry.tagline === 'string',
  )
}

// ---- External guests (Rust security/guest.rs) -----------------------------------------------------------------------

/** Email a guest a sign-in code (public door). Rust rate-limits per address and per IP. */
export async function rustApiRequestGuestCode(email: string, clientIp: string | null): Promise<void> {
  const correlationId = randomUUID()
  await rustApiPost('/v1/security/guest-code', buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId }), correlationId, { email, clientIp })
}

/** Check a guest's code (public door). On success the guest exists, and the answer is the email to sign in as. */
export async function rustApiVerifyGuestCode(email: string, code: string): Promise<{ email: string }> {
  const correlationId = randomUUID()
  return rustApiPost('/v1/security/guest-code/verify', buildRustPublicBridgeHeaders({ internalApiKey: internalApiKey(), correlationId }), correlationId, { email, code })
}

/**
 * Provision (first sign-in) and resolve the guest behind a signed-in identity, answered like `rustApiResolveIdentity`.
 * A staff identity that already maps is only resolved; Rust creates EXTERNAL guests only.
 */
export async function rustApiProvisionGuest(
  identity: { provider: string; providerSubject: string },
  claim: { email: string | null; emailVerified: boolean; displayName: string | null },
): Promise<RustIdentityResolution> {
  const correlationId = randomUUID()
  const headers = buildRustBridgeHeaders({ identity, internalApiKey: internalApiKey(), correlationId })
  return rustApiPost('/v1/security/guests', headers, correlationId, claim)
}

/** POST JSON to Rust; a refusal (bad code, rate limit) arrives as a RustApiError with Rust's code and a 4xx status. */
async function rustApiPost<T>(path: string, headers: Record<string, string>, correlationId: string, body: unknown): Promise<T> {
  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method: 'POST',
      headers: { ...headers, 'content-type': 'application/json' },
      body: JSON.stringify(body),
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }
  const payload = (await response.json().catch(() => null)) as RustApiSuccess<T> | RustApiFailure | null
  if (!payload) {
    throw new RustApiError({ status: 502, code: 'RUST_API_INVALID_RESPONSE', message: 'Rust API returned a non-JSON response.', retryable: true, correlationId })
  }
  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }
  return payload.value
}

function rustApiBaseUrl(): string {
  const value = resolveRustApiBaseUrl(process.env.RUST_API_BASE_URL, process.env.NODE_ENV)
  if (value) return value
  throw new RustApiError({
    status: 503,
    code: 'RUST_API_UNAVAILABLE',
    message: 'RUST_API_BASE_URL is not configured.',
    retryable: true,
  })
}

function internalApiKey(): string {
  const value = resolveInternalApiKey(
    process.env.CULEBRA_INTERNAL_API_KEY,
    process.env.AUTH_SECRET,
  )
  if (value) return value
  throw new RustApiError({
    status: 503,
    code: 'RUST_API_UNAVAILABLE',
    message: 'CULEBRA_INTERNAL_API_KEY is not configured.',
    retryable: false,
  })
}

/**
 * GET-only bridge from the authenticated Next/Auth.js edge to the Rust service.
 *
 * This deliberately has no generic write method. Write cutover happens only
 * after the read transport is proven and each command route is explicitly
 * reviewed.
 */
export async function rustApiRead<T>(
  path: `/v1/${string}`,
  options: RustApiReadOptions = {},
): Promise<RustApiSuccess<T>> {
  const identity =
    options.identity ??
    (await createAuthJsSessionAdapter().getSession()) ??
    // A bypassed dev session has no provider identity; this presents the real auth_identity for the bypass user so the
    // Rust API resolves the same application user it would for a signed-in caller. Returns null in production, by its
    // own guard, so this line cannot weaken anything that ships.
    (await bypassBridgeIdentity())
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = buildRustBridgeHeaders({
    identity,
    internalApiKey: internalApiKey(),
    correlationId,
    causationId: options.causationId,
  })

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method: 'GET',
      headers,
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<T> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<T> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }

  return payload
}


/**
 * JSON command bridge, restricted by its own type to the reviewed engine routes.
 *
 * The earlier rule here was "no generic write method", and the reason mattered: an open door to arbitrary writes would
 * let any caller cut a route over before anyone had reviewed it. That reason is preserved rather than dropped - the
 * path type below admits only `/v1/engine/...`, so this is not a general write door, it is the engine's door. Adding a
 * new engine command means adding a route on the Rust side and naming it in the union here, which is a review.
 *
 * Identity resolves exactly as it does for reads, bypass bridge included, so a dev session still attributes the command
 * to the real user the Rust side resolves.
 */
export type RustApiEnginePath =
  | '/v1/engine/transactions'
  | '/v1/engine/timers/reconcile'
  | '/v1/engine/tasks/complete'
  | '/v1/engine/reclaim'

export async function rustApiEngineCommand<T>(
  path: RustApiEnginePath,
  body: Record<string, unknown>,
  options: RustApiReadOptions = {},
): Promise<RustApiSuccess<T>> {
  const identity =
    (await createAuthJsSessionAdapter().getSession()) ?? (await bypassBridgeIdentity())
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = {
    ...buildRustBridgeHeaders({
      identity,
      internalApiKey: internalApiKey(),
      correlationId,
      causationId: options.causationId,
    }),
    'content-type': 'application/json',
  }

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method: 'POST',
      headers,
      body: JSON.stringify(body),
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<T> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<T> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }

  return payload
}


/**
 * Multipart write bridge for commands that are already authenticated and
 * validated at the Next edge. Rust re-resolves the canonical user and owns the
 * business transaction.
 */
export async function rustApiWriteForm<T>(
  path: `/v1/${string}`,
  formData: FormData,
  options: RustApiReadOptions = {},
): Promise<RustApiSuccess<T>> {
  const identity = await createAuthJsSessionAdapter().getSession()
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = buildRustBridgeHeaders({
    identity,
    internalApiKey: internalApiKey(),
    correlationId,
    causationId: options.causationId,
  })

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method: 'POST',
      headers,
      body: formData,
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<T> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<T> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }

  return payload
}


type RustApiJsonWriteOptions = RustApiReadOptions

async function rustApiJsonWrite<T>(
    path: `/v1/${string}`,
  method: 'POST' | 'PATCH' | 'PUT',
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  const identity =
    (await createAuthJsSessionAdapter().getSession()) ??
    (await bypassBridgeIdentity())
  if (!identity) {
    throw new RustApiError({
      status: 401,
      code: 'AUTH_IDENTITY_REQUIRED',
      message: 'An authenticated provider identity is required.',
    })
  }

  const correlationId = options.correlationId?.trim() || randomUUID()
  const headers = {
    ...buildRustBridgeHeaders({
      identity,
      internalApiKey: internalApiKey(),
      correlationId,
      causationId: options.causationId,
    }),
    'content-type': 'application/json',
  }

  let response: Response
  try {
    response = await fetch(`${rustApiBaseUrl()}${path}`, {
      method,
      headers,
      body: JSON.stringify(body),
      cache: 'no-store',
    })
  } catch (cause) {
    throw new RustApiError({
      status: 503,
      code: 'RUST_API_UNAVAILABLE',
      message: cause instanceof Error ? cause.message : 'Rust API request failed.',
      retryable: true,
      correlationId,
    })
  }

  let payload: RustApiSuccess<T> | RustApiFailure
  try {
    payload = (await response.json()) as RustApiSuccess<T> | RustApiFailure
  } catch {
    throw new RustApiError({
      status: 502,
      code: 'RUST_API_INVALID_RESPONSE',
      message: 'Rust API returned a non-JSON response.',
      retryable: true,
      correlationId,
    })
  }

  if (!response.ok || !payload.ok) {
    const failure = payload as RustApiFailure
    throw new RustApiError({
      status: response.status,
      code: failure.error?.code ?? 'RUST_API_FAILURE',
      message: failure.error?.message ?? 'Rust API request failed.',
      retryable: failure.error?.retryable ?? response.status >= 500,
      correlationId: failure.correlationId ?? correlationId,
      incidentId: failure.error?.incidentId ?? null,
    })
  }

  return payload
}

export async function rustApiSetRoleEntitlement<T>(
  body: { roleCode: string; action: string; granted: boolean },
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/security/role-entitlements', 'PUT', body)
}

export async function rustApiSetUserPrimaryRole<T>(
  body: { appUserId: string; roleCode: string },
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/security/users', 'PUT', body)
}

export async function rustApiCreateDeal<T>(
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/deals', 'POST', body, options)
}

export async function rustApiDealCommand<T>(
  dealId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/deals/${encodeURIComponent(dealId)}/commands`) as `/v1/${string}`,
    'POST',
    body,
    options,
  )
}

export async function rustApiCreateForm<T>(
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/forms', 'POST', body, options)
}

export async function rustApiUpdateForm<T>(
  formId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/forms/${encodeURIComponent(formId)}`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}


export async function rustApiCreatePropertyAdmin<T>(
  body: { name: string; propertyType?: string | null },
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/properties/admin', 'POST', body, options)
}

export async function rustApiUpdatePropertyAdmin<T>(
  propertyId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/properties/${encodeURIComponent(propertyId)}/admin`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}

export async function rustApiCreatePropertyVideoUpload<T>(
  propertyId: string,
  body: { corsOrigin: string },
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/properties/${encodeURIComponent(propertyId)}/video-uploads`) as `/v1/${string}`,
    'POST',
    body,
    options,
  )
}

export async function rustApiFinalizePropertyVideoUpload<T>(
  propertyId: string,
  uploadId: string,
  body: { role: 'video' | 'short'; caption?: string | null },
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/properties/${encodeURIComponent(propertyId)}/video-uploads/${encodeURIComponent(uploadId)}/finalize`) as `/v1/${string}`,
    'POST',
    body,
    options,
  )
}

export async function rustApiAttachPropertyVideo<T>(
  propertyId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/properties/${encodeURIComponent(propertyId)}/video`) as `/v1/${string}`,
    'POST',
    body,
    options,
  )
}

export async function rustApiUpdatePersonAdmin<T>(
  personId: string,
  body: {
    displayName: string
    status: string
    company?: string | null
  },
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/people/${encodeURIComponent(personId)}`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}

export async function rustApiUpdateProject<T>(
  projectId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/projects/${encodeURIComponent(projectId)}`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}

export async function rustApiUpdateWbs<T>(
  itemId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/wbs/${encodeURIComponent(itemId)}`) as `/v1/${string}`,
    'PATCH',
    body,
    options,
  )
}

export async function rustApiRouteProjectWork<T>(
  itemId: string,
  body: Record<string, unknown>,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/wbs/${encodeURIComponent(itemId)}/route`) as `/v1/${string}`,
    'POST',
    body,
    options,
  )
}



export async function rustApiCompleteTask<T>(
  taskId: string,
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/tasks/${encodeURIComponent(taskId)}/complete`) as `/v1/${string}`,
    'POST',
    {},
    options,
  )
}

/**
 * Accounting V1's three commands.
 *
 * `amount` is a STRING on the way in, as it is on the way out: the digits the operator typed are the digits Postgres
 * stores. Passing a number here would round it before the service could validate it, which is the sort of loss that only
 * shows up as a reconciliation difference months later.
 */
export async function rustApiCreateExpense<T>(
  body: {
    vendor: string
    category: string
    amount: string
    expenseOn: string
    memo?: string | null
    dealId?: string | null
    propertyId?: string | null
    personId?: string | null
  },
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/accounting/expenses', 'POST', body, options)
}

export async function rustApiCreateReceivable<T>(
  body: {
    reference?: string | null
    description: string
    category: string
    amount: string
    issuedOn: string
    dueOn?: string | null
    dealId?: string | null
    propertyId?: string | null
    personId?: string | null
  },
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>('/v1/accounting/receivables', 'POST', body, options)
}

export async function rustApiMarkReceivablePaid<T>(
  receivableId: string,
  body: { paidOn: string },
  options: RustApiJsonWriteOptions = {},
): Promise<RustApiSuccess<T>> {
  return rustApiJsonWrite<T>(
    (`/v1/accounting/receivables/${encodeURIComponent(receivableId)}/paid`) as `/v1/${string}`,
    'POST',
    body,
    options,
  )
}
