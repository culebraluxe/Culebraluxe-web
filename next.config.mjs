/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    unoptimized: true,
  },
  // The Forms PDF composer reads the canonical brand PNG from the server
  // filesystem. Force that asset into every traced server function so Vercel
  // cannot omit it and fall back to the plain-text CULEBRALUXE header.
  outputFileTracingIncludes: {
    '/*': ['./public/brand/CLLOGO.png'],
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
