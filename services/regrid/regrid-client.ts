import type {
  RegridAddressLookupInput,
  RegridAddressLookupResult,
  RegridClientConfig,
  RegridFields,
  RegridParcel,
  RegridParcelFeature,
} from './types'

const DEFAULT_BASE_URL = 'https://app.regrid.com/api/v2'
const DEFAULT_PATH = '/us/pr'
const DEFAULT_TIMEOUT_MS = 10_000
const DEFAULT_LIMIT = 2

function text(value: unknown): string | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null
  const normalized = String(value).trim()
  return normalized || null
}

function numberValue(value: unknown): number | null {
  if (value === null || value === undefined || value === '') return null
  const parsed = typeof value === 'number' ? value : Number(String(value).replace(/,/g, ''))
  return Number.isFinite(parsed) ? parsed : null
}

function firstNumber(fields: RegridFields, keys: string[]): number | null {
  for (const key of keys) {
    const value = numberValue(fields[key])
    if (value !== null) return value
  }
  return null
}

function firstText(fields: RegridFields, keys: string[]): string | null {
  for (const key of keys) {
    const value = text(fields[key])
    if (value !== null) return value
  }
  return null
}

/** Normalize one Regrid GeoJSON feature into the small set of facts CulebraLuxe promotes. */
export function normalizeRegridParcel(feature: RegridParcelFeature): RegridParcel {
  const properties = feature.properties ?? {}
  const fields = properties.fields ?? {}
  const path = text(properties.path) ?? text(fields.path)
  const llUuid = text(properties.ll_uuid) ?? text(fields.ll_uuid)

  return {
    llUuid,
    parcelNumber: firstText(fields, ['parcelnumb', 'state_parcelnumb', 'tax_id']),
    path,
    headline: text(properties.headline),
    situsAddress: firstText(fields, ['address', 'original_address']),
    owner: firstText(fields, ['owner', 'unmodified_owner']),
    urbanization: text(fields.urbanization),
    neighborhood: text(fields.neighborhood),
    zoning: text(fields.zoning),
    zoningDescription: text(fields.zoning_description),
    legalDescription: text(fields.legaldesc),
    latitude: numberValue(fields.lat),
    longitude: numberValue(fields.lon),
    lotAcres: firstNumber(fields, ['deeded_acres', 'gisacre', 'll_gisacre']),
    lotSquareFeet: firstNumber(fields, ['sqft', 'll_gissqft']),
    // Prefer the assessor's habitable/taxable area, then its broader building-area field.
    buildingSquareFeet: firstNumber(fields, ['recrdareano', 'area_building', 'll_bldg_footprint_sqft']),
    yearBuilt: firstNumber(fields, ['yearbuilt']),
    stories: firstNumber(fields, ['numstories']),
    bedrooms: firstNumber(fields, ['num_bedrooms']),
    bathrooms: firstNumber(fields, ['num_bath']),
    parcelValue: firstNumber(fields, ['parval']),
    landValue: firstNumber(fields, ['landval']),
    improvementValue: firstNumber(fields, ['improvval']),
    fields,
    geometry: feature.geometry ?? null,
    rawFeature: feature,
  }
}

function safeBaseUrl(value: string | undefined): string {
  return (value?.trim() || DEFAULT_BASE_URL).replace(/\/+$/, '')
}

export class RegridClient {
  private readonly token: string
  private readonly baseUrl: string
  private readonly defaultPath: string | null
  private readonly timeoutMs: number
  private readonly fetchImpl: typeof fetch

  constructor(config: RegridClientConfig) {
    const token = config.token.trim()
    if (!token) throw new Error('REGRID_API_TOKEN is required')
    this.token = token
    this.baseUrl = safeBaseUrl(config.baseUrl)
    this.defaultPath = config.defaultPath === undefined ? DEFAULT_PATH : config.defaultPath
    this.timeoutMs = config.timeoutMs ?? DEFAULT_TIMEOUT_MS
    this.fetchImpl = config.fetchImpl ?? fetch
  }

  async lookupAddress(input: RegridAddressLookupInput): Promise<RegridAddressLookupResult> {
    const query = input.query.trim()
    if (!query) throw new Error('Regrid address query is required')

    // limit=2 intentionally uses ONE request while still detecting ambiguity.
    const limit = Math.max(1, Math.min(input.limit ?? DEFAULT_LIMIT, 2))
    const path = input.path === undefined ? this.defaultPath : input.path
    const url = new URL(`${this.baseUrl}/parcels/address`)
    url.searchParams.set('query', query)
    if (path?.trim()) url.searchParams.set('path', path.trim())
    url.searchParams.set('limit', String(limit))
    url.searchParams.set('return_geometry', 'true')
    url.searchParams.set('return_custom', 'true')

    // Prefer the documented header credential so the API token never appears in
    // a URL, exception, proxy log, or app_error row.
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs)
    let response: Response
    try {
      response = await this.fetchImpl(url, {
        method: 'GET',
        headers: {
          accept: 'application/json',
          'x-regrid-token': this.token,
        },
        signal: controller.signal,
      })
    } catch (error) {
      if (controller.signal.aborted) {
        throw new Error(`Regrid address lookup timed out after ${this.timeoutMs}ms`)
      }
      const message = error instanceof Error ? error.message : String(error)
      throw new Error(`Regrid address lookup failed: ${message}`)
    } finally {
      clearTimeout(timeout)
    }

    if (!response.ok) {
      // Deliberately do not include response bodies or credential-bearing URLs.
      throw new Error(`Regrid address lookup failed with HTTP ${response.status}`)
    }

    const payload = (await response.json()) as {
      parcels?: { features?: RegridParcelFeature[] }
    }
    const features = Array.isArray(payload.parcels?.features) ? payload.parcels.features : []
    const matches = features.slice(0, limit).map(normalizeRegridParcel)

    if (matches.length === 0) {
      return { status: 'not_found', query, path: path?.trim() || null, matches: [] }
    }
    if (matches.length === 1) {
      return {
        status: 'matched',
        query,
        path: path?.trim() || null,
        parcel: matches[0],
        matches: [matches[0]],
      }
    }
    return { status: 'ambiguous', query, path: path?.trim() || null, matches }
  }
}

export function regridClientFromEnv(env: NodeJS.ProcessEnv = process.env): RegridClient {
  return new RegridClient({
    token: env.REGRID_API_TOKEN ?? '',
    baseUrl: env.REGRID_API_BASE_URL,
    defaultPath: env.REGRID_PATH?.trim() || DEFAULT_PATH,
  })
}
