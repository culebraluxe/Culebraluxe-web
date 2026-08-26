import { sql } from "@/db/client"
import { guardPortalUpload } from "@/lib/auth/portal-session"

// ---------------------------------------------------------------------------
// HARDEN-05 — Public media inherits Property publication state.
//
// PROPERTY OWNS PUBLICATION STATE; listing media does not carry independent
// public state. A media asset is publicly reachable only when every Property
// it is linked to (via property_media) is published (is_published = true) and
// not archived. Media with no Property link is not listing media and remains
// reachable. Authenticated portal viewers (portal.read) may still fetch
// internal-Property media; that internal traffic is cached `private` so it can
// never leak through a public/CDN cache.
// ---------------------------------------------------------------------------

export async function GET(
  request: Request,
  context: { params: Promise<{ id: string }> }
) {
  const { id } = await context.params

  // Authenticated portal escape hatch. On any failure (anonymous, wrong
  // authority, or unexpected) we fail closed to the public publication gate.
  let authed = false
  try {
    const guard = await guardPortalUpload("portal.read")
    authed = guard.ok
  } catch {
    authed = false
  }

  const result = await sql`
    SELECT
      m.file_data,
      m.mime_type,
      COALESCE(
        (
          SELECT BOOL_AND(p.is_published = true AND p.archived_at IS NULL)
          FROM property_media pm
          JOIN property p ON p.id = pm.property_id
          WHERE pm.media_id = m.id
        ),
        true
      ) AS publicly_allowed
    FROM media m
    WHERE m.id = ${id}
    LIMIT 1
  `

  if (result.length === 0) {
    return new Response("Not found", { status: 404 })
  }

  const row = result[0]
  const allowed = authed || row.publicly_allowed === true
  if (!allowed) {
    // 404 (not 403) so internal media ids are not distinguishable.
    return new Response("Not found", { status: 404 })
  }

  return new Response(row.file_data, {
    headers: {
      "Content-Type": row.mime_type,
      "Cache-Control": authed ? "private, no-store" : "public, max-age=3600",
    },
  })
}