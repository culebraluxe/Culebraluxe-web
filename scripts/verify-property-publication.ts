// HARDEN-05 — DEV runtime proof of the Property publication / market
// visibility invariant against the DEV control plane.
//
// PROPERTY OWNS PUBLICATION STATE; LISTING MEDIA INHERITS IT.
//
// Proves:
//   - one INTERNAL (is_published=false) and one PUBLIC (is_published=true)
//   - INTERNAL: visible internally, absent from all public reads, direct URL
//     unavailable, media not publicly surfaced
//   - PUBLIC: visible internally, present in public reads, detail works,
//     media publicly surfaced
//   - toggle PUBLIC->INTERNAL removes public visibility; INTERNAL->PUBLIC
//     restores it (the toggled property is restored to published at the end)
//
// Run: node --env-file=.env.local --import tsx scripts/verify-property-publication.ts

import { sql } from '../db/client'
import {
  getFilteredProperties,
  getProperties,
  getPropertyBySlug,
  getPublicPropertySlugs,
  getPropertyIntroById,
} from '../db/properties'
import { setPropertyPublished } from '../db/portal-property'

let failures = 0
function check(name: string, ok: boolean, extra = '') {
  console.log(`${ok ? 'ok' : 'FAIL'} - ${name}${extra ? ' :: ' + extra : ''}`)
  if (!ok) failures++
}

type Prop = {
  id: string
  slug: string | null
  status: string
  is_published: boolean
  archived_at: string | null
}

async function publicReads(): Promise<{ ids: Set<string>; slugs: Set<string> }> {
  const [allR, filteredR, slugsR] = await Promise.all([
    getProperties({ publicOnly: true }),
    getFilteredProperties({}),
    getPublicPropertySlugs(),
  ])
  const all = allR.ok ? allR.data : []
  const filtered = filteredR.ok ? filteredR.data : { properties: [], viewOptions: [] }
  const slugs = slugsR.ok ? slugsR.data : []
  const ids = new Set<string>()
  for (const p of [...all, ...filtered.properties]) ids.add(p.id)
  return { ids, slugs: new Set(slugs) }
}

async function mediaPubliclyAllowed(propertyId: string): Promise<boolean> {
  // Mirrors the /api/media/[id] anonymous gate (media inherits publication).
  const rows = await sql`
    select
      coalesce(
        (select bool_and(p.is_published = true and p.archived_at is null)
         from property_media pm join property p on p.id = pm.property_id
         where pm.media_id = m.id),
        true
      ) as publicly_allowed
    from media m
    join property_media pm2 on pm2.media_id = m.id
    where pm2.property_id = ${propertyId}
    limit 1
  `
  return rows.length === 0 ? true : Boolean(rows[0]?.publicly_allowed)
}

async function propertyHasMedia(propertyId: string): Promise<boolean> {
  const rows = await sql`
    select 1 from property_media where property_id = ${propertyId} limit 1
  `
  return rows.length > 0
}

async function main() {
  const props = (await sql`
    select id, slug, status, is_published, archived_at
    from property
    where archived_at is null
    order by is_published desc, slug is not null desc, created_at asc
  `) as Prop[]

  const pub = props.find((p) => p.is_published && p.slug)
  // The internal candidate may lack a slug (a staged property not yet
  // addressable). Direct-URL 404 is proven separately via the slugged toggle.
  const priv = props.find((p) => !p.is_published)
  check('found a PUBLIC property', Boolean(pub), pub?.id ?? '')
  check('found an INTERNAL property', Boolean(priv), priv?.id ?? '')
  if (!pub || !priv) {
    failures++
    return
  }

  // ---- INTERNAL: visible internally, absent publicly, media not public ----
  const privIntro = await getPropertyIntroById(priv.id)
  check('INTERNAL visible internally', Boolean(privIntro.ok && privIntro.data !== null), priv.slug ?? '(no slug)')
  const privPub = await publicReads()
  check('INTERNAL absent from public collection', !privPub.ids.has(priv.id))
  if (priv.slug) {
    check('INTERNAL absent from public slugs', !privPub.slugs.has(priv.slug))
    const privDetail = await getPropertyBySlug(priv.slug)
    check('INTERNAL direct URL unavailable (null -> 404)', privDetail.ok && privDetail.data === null)
  }
  if (await propertyHasMedia(priv.id)) {
    check('INTERNAL media not publicly surfaced', !(await mediaPubliclyAllowed(priv.id)))
  } else {
    check('INTERNAL media not publicly surfaced', true, '(no media linked)')
  }

  // ---- PUBLIC: visible internally + publicly, detail + media work ----
  const pubIntro = await getPropertyIntroById(pub.id)
  check('PUBLIC visible internally', Boolean(pubIntro.ok && pubIntro.data !== null))
  const pubPub = await publicReads()
  check('PUBLIC present in public collection', pubPub.ids.has(pub.id))
  check('PUBLIC present in public slugs', pubPub.slugs.has(pub.slug!))
  const pubDetail = await getPropertyBySlug(pub.slug!)
  check('PUBLIC detail available', pubDetail.ok && pubDetail.data !== null)
  check('PUBLIC media publicly surfaced', await mediaPubliclyAllowed(pub.id))

  // ---- Toggle PUBLIC -> INTERNAL (public removal) ----
  await setPropertyPublished(pub.id, false)
  const downPub = await publicReads()
  check('PUBLIC->INTERNAL removed from public collection', !downPub.ids.has(pub.id))
  check('PUBLIC->INTERNAL removed from public slugs', !downPub.slugs.has(pub.slug!))
  const downDetail = await getPropertyBySlug(pub.slug!)
  check('PUBLIC->INTERNAL direct URL 404', downDetail.ok && downDetail.data === null)
  check('PUBLIC->INTERNAL media no longer public', !(await mediaPubliclyAllowed(pub.id)))
  const downIntro = await getPropertyIntroById(pub.id)
  check('PUBLIC->INTERNAL still visible internally', Boolean(downIntro.ok && downIntro.data !== null))

  // ---- Toggle INTERNAL -> PUBLIC (public visibility) ----
  await setPropertyPublished(pub.id, true)
  const upPub = await publicReads()
  check('INTERNAL->PUBLIC back in public collection', upPub.ids.has(pub.id))
  check('INTERNAL->PUBLIC back in public slugs', upPub.slugs.has(pub.slug!))
  const upDetail = await getPropertyBySlug(pub.slug!)
  check('INTERNAL->PUBLIC detail available', upDetail.ok && upDetail.data !== null)
  check('INTERNAL->PUBLIC media public again', await mediaPubliclyAllowed(pub.id))

  console.log(
    failures === 0 ? '\nVERIFY PASSED' : `\nVERIFY FAILED (${failures})`,
  )
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
