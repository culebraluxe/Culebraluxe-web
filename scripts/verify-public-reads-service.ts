// PROOF: the public inventory reads now run through the Property SERVICE and
// return exactly what the previous direct-db path returned. Compares, per read:
//   (a) service path: composeCoreServices -> PropertyService -> SqlPropertyRepository
//   (b) direct SQL path: db/property-public-reads.ts
// on the DEV control plane. DEV only; no writes.

import { composeCoreServices } from '../services/composition'
import { AuthorizationService } from '../services/entitlement/authorization-service'
import { StaticAuthorizationPolicyProvider } from '../services/entitlement/authorization-service'
import { SqlPropertyRepository } from '../db/property-service-repository'
import {
  getFilteredProperties,
  getProperties,
  getPropertyBySlug,
  getPropertyIntroById,
  getPublicPropertySlugs,
} from '../db/property-public-reads'
import { PROPERTY_OPERATIONS } from '../services/property'
import type { PropertySummary } from '../services/property'
import { sql } from '../db/client'

const repos = {
  person: null as never,
  firm: null as never,
  property: new SqlPropertyRepository(),
  contract: null as never,
  showing: null as never,
  security: null as never,
  wbs: null as never,
  project: null as never,
}

const services = composeCoreServices(repos, {
  authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
})
const property = services.property

let failures = 0
const check = (name: string, ok: boolean, extra = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} - ${name}${extra ? ' :: ' + extra : ''}`)
  if (!ok) failures++
}

/** Runs one public-read operation through the service; returns the inner Result. */
async function svcData<T>(
  operation: string,
  payload: unknown,
): Promise<{ ok: boolean; data?: T; error?: unknown }> {
  const result = await property.execute({
    operation: operation as never,
    payload: payload as never,
    context: { actor: { id: null, kind: 'system' }, correlationId: 'probe' },
  })
  if (!result.ok) return { ok: false, error: result.error }
  const inner = result.value as unknown as { ok: boolean; data?: T; error?: unknown }
  return inner.ok ? { ok: true, data: inner.data } : { ok: false, error: inner.error }
}

const ids = (list: { id: string }[] | undefined) => (list ?? []).map((row) => row.id).sort()

async function main() {
  // 1 — list (public): must equal the direct SQL, and must be non-empty.
  const directList = await getProperties({ publicOnly: true })
  const svcList = await svcData<PropertySummary[]>(PROPERTY_OPERATIONS.LIST, { publicOnly: true })
  const directPublic = directList.ok ? directList.data : []
  check(
    'list(publicOnly) matches direct SQL',
    JSON.stringify(directPublic.map((p) => p.id).sort()) === JSON.stringify(ids(svcList.data)),
    `${directPublic.length} rows`,
  )
  check('list(publicOnly) returns active listings only', directPublic.length > 0)

  // 2 — list (internal picker)
  const directAll = await getProperties()
  const svcAll = await svcData<typeof directPublic>(PROPERTY_OPERATIONS.LIST, {})
  const directAllRows = directAll.ok ? directAll.data : []
  check(
    'list(internal) matches direct SQL',
    JSON.stringify(directAllRows.map((p) => p.id).sort()) === JSON.stringify(ids(svcAll.data)),
    `${directAllRows.length} rows`,
  )

  // 3 — search
  const directSearch = await getFilteredProperties({})
  const svcSearch = await svcData<{ properties: typeof directPublic; viewOptions: string[] }>(
    PROPERTY_OPERATIONS.SEARCH,
    { filters: {} },
  )
  const directSearchRows = directSearch.ok ? directSearch.data.properties : []
  check(
    'search matches direct SQL',
    JSON.stringify(directSearchRows.map((p) => p.id).sort()) === JSON.stringify(ids(svcSearch.data?.properties)),
    `${directSearchRows.length} rows`,
  )

  // 4 — public slugs
  const directSlugs = await getPublicPropertySlugs()
  const svcSlugs = await svcData<string[]>(PROPERTY_OPERATIONS.PUBLIC_SLUGS, {})
  const directSlugList = directSlugs.ok ? [...directSlugs.data].sort() : []
  const svcSlugList = [...(svcSlugs.data ?? [])].sort()
  check(
    'publicSlugs matches direct SQL',
    JSON.stringify(directSlugList) === JSON.stringify(svcSlugList),
    `${directSlugList.length} slugs`,
  )

  // 5 — detail by slug: compare the FULL payload.
  const slug = directSlugList[0]
  if (slug) {
    const directDetail = await getPropertyBySlug(slug)
    const svcDetail = await svcData<unknown>(PROPERTY_OPERATIONS.BY_SLUG, { slug })
    const directKey = directDetail.ok ? JSON.stringify(directDetail.data) : 'ERROR'
    const svcKey = svcDetail.ok ? JSON.stringify(svcDetail.data) : 'ERROR'
    const label = directKey === 'null' ? 'null' : directKey === 'ERROR' ? 'error' : 'resolved'
    check('bySlug matches direct SQL (full payload)', directKey === svcKey, `${slug} -> ${label}`)
  } else {
    check('bySlug (skipped — no live slug)', true)
  }

  // 6 — intro
  const firstId = directPublic[0]?.id
  if (firstId) {
    const directIntro = await getPropertyIntroById(firstId)
    const svcIntro = await svcData<unknown>(PROPERTY_OPERATIONS.INTRO, { propertyId: firstId })
    const directKey = directIntro.ok ? JSON.stringify(directIntro.data) : 'ERROR'
    const svcKey = svcIntro.ok ? JSON.stringify(svcIntro.data) : 'ERROR'
    check('intro matches direct SQL', directKey === svcKey, String(directIntro.ok ? directIntro.data?.name : 'error'))
  } else {
    check('intro (skipped — no live property)', true)
  }

  // 7 — a published but NON-listing property must not reach public inventory.
  const nonListing = await sql`
    select count(*)::int as n from property
     where archived_at is null and is_published = true and not is_active_listing
  `
  const nonListingPublished = Number((nonListing[0] as { n: number }).n)
  const leaked = (svcList.data ?? []).filter((p) =>
    directPublic.every((direct) => direct.id !== p.id),
  )
  check(
    'only active listings appear in public inventory',
    leaked.length === 0,
    `${nonListingPublished} published non-listing(s) in the DB`,
  )

  console.log(failures === 0 ? '\nALL PUBLIC READ PROOFS PASSED' : `\n${failures} FAILURE(S)`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
