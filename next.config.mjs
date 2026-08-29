/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    unoptimized: true,
  },
  // REVIEW-BRANCH ONLY: main currently has two known, pre-existing type errors
  // in lib/decision-analysis/pdf.ts (TimesBold / TimesItalic). Ignore type errors
  // here only so the nonproduction Listing v2 PDF dry run can deploy. This
  // setting must NOT be merged to main.
  typescript: {
    ignoreBuildErrors: true,
  },
  // ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
  //    LONGER NEEDED. Review responses must never be indexed or followed.
  async headers() {
    return [
      {
        source: '/review/:path*',
        headers: [
          { key: 'X-Robots-Tag', value: 'noindex, nofollow' },
        ],
      },
    ]
  },
}

export default nextConfig
