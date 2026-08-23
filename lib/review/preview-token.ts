import { timingSafeEqual } from "node:crypto"

// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Server-side-only guard for the /review/<TOKEN>/<PAGE> preview prefix. The
// token is compared against PORTAL_REVIEW_TOKEN (Vercel env) in constant time.
// Failure returns 404 (never a login redirect, never a hint that the route
// exists). The token is never rendered into client JS by the preview pages.
// ═══════════════════════════════════════════════════════════════════════════

export function isReviewTokenValid(
  supplied: string | null | undefined,
): boolean {
  const expected = process.env.PORTAL_REVIEW_TOKEN
  if (!expected || !supplied) return false
  const a = Buffer.from(supplied, "utf8")
  const b = Buffer.from(expected, "utf8")
  if (a.length !== b.length) return false
  return timingSafeEqual(a, b)
}
