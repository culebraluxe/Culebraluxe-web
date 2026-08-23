// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Temporary, read-only, UNLISTED preview path for external architect review:
//
//   /review/<PORTAL_REVIEW_TOKEN>/design-lab
//   /review/<PORTAL_REVIEW_TOKEN>/storyboard
//   /review/<PORTAL_REVIEW_TOKEN>/dashboard
//   /review/<PORTAL_REVIEW_TOKEN>/command-center
//
// - Token is compared server-side against PORTAL_REVIEW_TOKEN (Vercel env);
//   bad/missing token and unsupported pages all return 404 (no login
//   redirect, no clue the route exists). No token is ever rendered to the
//   client.
// - This bypass applies ONLY to this exact /review prefix. Normal /portal/*
//   auth (middleware matcher `/portal/:path*` + portal layout) is untouched.
// - Responses carry noindex/nofollow (metadata robots + X-Robots-Tag header
//   from next.config.mjs). No navigation entry, no sitemap.
// - Every preview renderer is read-only: no server actions, no mutation
//   controls. See components/review/*.
// ═══════════════════════════════════════════════════════════════════════════

import type { Metadata } from "next"
import { notFound } from "next/navigation"

import { ReviewBanner } from "@/components/review/review-banner"
import { ReviewCommandCenter } from "@/components/review/review-command-center"
import { ReviewDashboard } from "@/components/review/review-dashboard"
import { ReviewDesignLab } from "@/components/review/review-design-lab"
import { ReviewStoryBoard } from "@/components/review/review-storyboard"
import { isReviewTokenValid } from "@/lib/review/preview-token"

export const dynamic = "force-dynamic"

export const metadata: Metadata = {
  robots: { index: false, follow: false },
}

// Explicit allowlist — never route arbitrary /portal/* paths through here.
const PREVIEW_PAGES = [
  "design-lab",
  "storyboard",
  "dashboard",
  "command-center",
] as const
type PreviewPage = (typeof PREVIEW_PAGES)[number]

export default async function ReviewPreviewPage({
  params,
}: {
  params: Promise<{ token: string; page: string }>
}) {
  const { token, page } = await params

  if (!isReviewTokenValid(token)) notFound()
  if (!(PREVIEW_PAGES as readonly string[]).includes(page)) notFound()

  const resolved = page as PreviewPage

  return (
    <div className="mx-auto max-w-7xl px-6 py-10">
      <ReviewBanner />
      {resolved === "design-lab" ? <ReviewDesignLab /> : null}
      {resolved === "storyboard" ? <ReviewStoryBoard /> : null}
      {resolved === "dashboard" ? <ReviewDashboard /> : null}
      {resolved === "command-center" ? <ReviewCommandCenter /> : null}
    </div>
  )
}
