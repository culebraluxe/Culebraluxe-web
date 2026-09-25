import { sql } from "@/legacy/db/client"

import { captureServerError } from '@/lib/server-error-capture'
import { getToken } from "next-auth/jwt"
import { withApiHandler } from '@/lib/error-capture-seam'

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

async function GETHandler(
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

  let result: Array<Record<string, unknown>>
  try {
    // THE COPY, NOT THE ORIGINAL. A photograph from a phone is 8–13 MB and the gateway refuses a response body over
    // ~4.5 MB — the same cap that refused the upload. Serving `m.file_data` for a large original would upload a photo
    // successfully and then show a broken image, which is worse than the original failure: the photo looks lost.
    // The `web` derivative (migration 222) is what a browser is handed; the original stays stored as the record.
    // The publication gate below still keys on the ORIGINAL, because the copies hang off it by `derivative_of`.
    result = await sql`
      SELECT
        COALESCE(copy.file_data, m.file_data) AS file_data,
        COALESCE(copy.mime_type, m.mime_type) AS mime_type,
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
      LEFT JOIN LATERAL (
        SELECT d.file_data, d.mime_type
        FROM media d
        WHERE d.derivative_of = m.id AND d.derivative_kind = 'web'
        LIMIT 1
      ) AS copy ON true
      WHERE m.id = ${id}
      LIMIT 1
    `
  } catch (err) {
    captureServerError('/api/media/[id]', err, { route: '/api/media/[id]' })
    // DB-HARDEN-01C — public media read: contain DB failure as a controlled
    // 503 (no SQL/stack leak, no global impact).
    console.error(
      '[media:gateway] public media read failed',
      err instanceof Error ? err.message : 'unknown',
    )
    return new Response("Service Unavailable", { status: 503 })
  }

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

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/media/[id]', route: '/api/media/[id]' },
  GETHandler,
)
