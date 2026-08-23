// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Minimal static probe: /review/<PORTAL_REVIEW_TOKEN>/probe
//   - correct token -> 200, body "ARCHITECT REVIEW OK", Content-Type text/plain
//   - wrong/missing token -> 404 (no leak, no login redirect)
// Reuses the existing isReviewTokenValid guard. No DB, no portal components,
// no React UI. noindex/nofollow is inherited from next.config.mjs headers for
// the /review/:path* prefix.
// ═══════════════════════════════════════════════════════════════════════════

import { isReviewTokenValid } from "@/lib/review/preview-token"

export const dynamic = "force-dynamic"

export async function GET(
  _req: Request,
  { params }: { params: Promise<{ token: string }> },
): Promise<Response> {
  const { token } = await params

  if (!isReviewTokenValid(token)) {
    return new Response("Not Found", { status: 404 })
  }

  return new Response("ARCHITECT REVIEW OK", {
    status: 200,
    headers: { "Content-Type": "text/plain" },
  })
}
