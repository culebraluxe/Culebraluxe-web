import { sql } from "@/db/client"
import { getToken } from "next-auth/jwt"

// ---------------------------------------------------------------------------
// HARDEN-05 + AUTH-BOUNDARY — Public media inherits Property publication state.
//
// PROPERTY OWNS PUBLICATION STATE; listing media does not carry independent
// public state. A media asset is publicly reachable only when every Property
// it is linked to (via property_media) is published (is_published = true) and
// not archived. Media with no Property link is not listing media and remains
// reachable.
//
// AUTH-BOUNDARY — this is a PUBLIC route. It must never construct Auth.js or
// depend on Portal security modules, so a broken Auth.js configuration (e.g.
// MissingSecret) cannot take down public media. The authenticated-portal
// escape hatch is implemented with the lightweight JWT decoder (next-auth/jwt)
// only — no `@/auth` import, no database identity lookup. When AUTH_SECRET is
// absent (or decoding fails) we fail closed to the publication gate.
// ---------------------------------------------------------------------------

export async function GET(
  request: Request,
  context: { params: Promise<{ id: string }> }
) {
  const { id } = await context.params

  // Authenticated portal escape hatch — JWT-only. Does NOT initialize Auth.js
  // and does not hit the DB, so it cannot affect public media when the Portal
  // auth subsystem is broken. Any authenticated session (portal.read holder)
  // may fetch internal-Property media; anonymous requests use the gate below.
  const secret = process.env.AUTH_SECRET
  let authed = false
  if (secret) {
    const token = await getToken({ req: request, secret }).catch(() => null)
    authed = Boolean(token?.sub)
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

  const row = result[0] as
    | {
        file_data: Buffer | Uint8Array | null
        mime_type: string
        publicly_allowed: boolean
      }
    | undefined
  if (!row) {
    return new Response("Not found", { status: 404 })
  }

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