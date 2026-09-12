/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    unoptimized: true,
  },
  // ForgeDB (db/forge-db.ts) is the application's single database pool, built on
  // node-postgres. `pg` must stay a RUNTIME dependency of the server bundle, not
  // something webpack inlines: it dynamically requires optional native and
  // connection-string modules that only resolve in a real Node runtime. Bundling it
  // is how you get a build that succeeds and a runtime that cannot connect.
  serverExternalPackages: ['pg'],
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
