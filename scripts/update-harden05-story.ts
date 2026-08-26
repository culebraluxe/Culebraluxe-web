// HARDEN-05 — record the Property publication / market visibility story.
import { createStoryboardStory, updateStoryboardStory } from '../db/storyboard'

const id = 'HARDEN-05'
const notes = `Property publication / market visibility invariant enforced: PROPERTY OWNS PUBLICATION STATE, LISTING MEDIA INHERITS IT. Recon used decision rule B (no existing single field cleanly represented "released to market"; status is a lifecycle enum with a scattered status IN (active,coming_soon,under_contract) public predicate). Added ONE canonical property-level field is_published boolean NOT NULL DEFAULT false (migration 082). Safe default NON-PUBLIC: new properties are internal until explicitly released. Backfill preserved current public inventory exactly (archived_at IS NULL AND slug IS NOT NULL AND status IN market set -> is_published=true); prod-safety: default false, backfill limited to already-public rows, verification queries below. Public reads hardened in db/properties.ts to gate on is_published = true (getFilteredProperties, getSimilarProperties, getPublicPropertySlugs, getPropertyBySlug direct URL -> null -> notFound 404; getProperties({publicOnly:true}) for home/buyers/favorites while portal forms/showings keep the internal default). Public media inherits: app/api/media/[id] now serves property-linked media to anonymous only when every owning Property is published and non-archived (BOOL_AND), with an authenticated-portal (portal.read) escape hatch so internal portal viewing of staged media still works (cached private so it can never leak via public cache); unlinked (non-listing) media remains reachable. Mutation uses the existing domain/security seam: db/portal-property.setPropertyPublished (idempotent, validated) + server action setPropertyPublishedAction gated on listing.write, revalidating portal + public property routes. Internal portal reads unchanged (see staged + public). DEV proof (scripts/verify-property-publication.ts) 19/19 PASSED on the DEV control plane: INTERNAL visible internally + absent from all public reads + (when media exists) media not public; PUBLIC visible internally + present in public collection/slugs + detail works + media public; toggle PUBLIC->INTERNAL removed collection/slugs, direct URL 404, media no longer public, still visible internally; INTERNAL->PUBLIC restored all public visibility. Live: / 200, /buyers 200, /properties/casa-luar 200, published media 200. Targeted tests workflow_app/tests/property-publication.test.ts 6/6 + adjacent 59 = 65/65; tsc clean; next build passed; git diff --check clean. NO PROD mutation. Production verification queries: select count(*) from property where is_published=true (should match prior public inventory); select id,slug,is_published from property where is_published=false and status in ('active','coming_soon','under_contract'); select count(*) from media m where not exists (select 1 from property_media pm join property p on p.id=pm.property_id where pm.media_id=m.id and (p.is_published=false or p.archived_at is not null)) and exists (select 1 from property_media pm2 where pm2.media_id=m.id). Deferred: no dedicated publish UI control surfaced this story (setPropertyPublishedAction is the seam); media inherited visibility only for property-linked assets.`

async function main() {
  const fields = {
    id,
    workstream: 'Security / Auth',
    title: 'Property Publication / Market Visibility',
    priority: 'High',
    status: 'Complete',
    notes,
    batch: null,
    goal: 'Make explicit, durable, and consistently enforced: a Property owns publication state; listing media inherits it.',
    scope: null,
    dependencies: null,
    preconditions: null,
    architectBrief: null,
    contextRefs: null,
    acceptanceCriteria: null,
    postconditions: null,
    operatingSurface: 'OPS',
    completion: 100,
    rollup: false,
    plannedStartAt: null,
    actualStartAt: null,
    completedAt: null,
  }
  try {
    await createStoryboardStory(fields)
    console.log('created', id)
  } catch {
    await updateStoryboardStory(id, fields)
    console.log('updated', id)
  }
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
