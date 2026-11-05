-- Guide photographs are PUBLIC content on a public page, but the object-storage migration promoted only media
-- reached through `property_media`: all 73 guide images (and their 219 size copies) stayed in
-- `culebraluxe-property-media-private` with `visibility = 'private'`. The guarded public door (`GET /api/media/{id}`,
-- `web/src/public_listings.rs::resolve_public_url`) requires a public-bucket, public-visibility, VERIFIED copy, so it
-- answered 404 and `/guide` showed no photographs (2026-11-06). The guide page is public and its images always were.
--
-- THE ROWS ARE PROMOTED HERE. The OBJECTS must also exist in the public bucket under the same keys: that copy is a
-- storage operation, not SQL. On production it was done with the media credential
-- (`culebraluxe-property-media-private` -> `culebraluxe-property-media-public`, 292 objects, each copied with the
-- source object's own `Content-Type` so the browser renders it). A database that has these rows but not the objects
-- answers the redirect with a missing object instead of an image.
--
-- Idempotent: re-running only rewrites rows that are already public.
update media
   set object_bucket = 'property_media_public',
       visibility = 'public',
       updated_at = now()
 where ((id in (select media_id from guide_item_media))
     or (derivative_of in (select media_id from guide_item_media)))
   and (object_bucket is distinct from 'property_media_public'
     or visibility is distinct from 'public');
