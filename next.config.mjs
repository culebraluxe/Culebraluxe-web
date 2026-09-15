/** @type {import('next').NextConfig} */
import { execSync } from 'node:child_process'

// ---------------------------------------------------------------------------
// BUILD STAMP — the artifact names its own source.
//
// Production is built LOCALLY (`scripts/vercel-build-prod.sh`) and deployed PREBUILT
// (`vercel deploy --prebuilt --prod`). Two consequences, both measured on 2026-09-14:
//
//   1. Next does NOT inline `NEXT_PUBLIC_*` into SERVER code - the built server chunk kept the
//      literal string `NEXT_PUBLIC_COCKPIT_SHA` and read `process.env` at request time. A locally
//      built artifact never has the build shell's environment at runtime, so the Cockpit's version
//      corner would have read `sha unknown` in production. (Caught by grepping the artifact before
//      deploying it, which is the only place this is visible.)
//   2. `next.config`'s `env` IS inlined into both client and server bundles at build time.
//
// So the commit and the build time are resolved HERE, while the build is running, and compiled into
// the code. `git rev-parse` makes this work even if nobody exports anything; the env vars (set by the
// build script) win when present, which keeps CI/platform builds working unchanged.
//
// What it is for: `V2 · <sha>` in the Cockpit's corner and `/api/build-info`, both compared against
// HEAD by the deploy script. Without it, "did the deploy actually go live?" has no answer.
// ---------------------------------------------------------------------------
function run(cmd) {
  try {
    return execSync(cmd, { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim()
  } catch {
    return ''
  }
}

const cockpitSha = process.env.NEXT_PUBLIC_COCKPIT_SHA || run('git rev-parse --short HEAD')
// A COUNTER THAT ONLY EVER GOES UP (the captain's ask): "we should increment that counter so we know we
// have the right build every time we change this". The commit count does exactly that - it changes on
// every commit, it is monotonic, and it needs no state to keep in sync.
const cockpitRevision = process.env.NEXT_PUBLIC_COCKPIT_REVISION || run('git rev-list --count HEAD')

const nextConfig = {
  env: {
    NEXT_PUBLIC_COCKPIT_SHA: cockpitSha,
    NEXT_PUBLIC_COCKPIT_REVISION: cockpitRevision,
    NEXT_PUBLIC_COCKPIT_BUILT_AT:
      process.env.NEXT_PUBLIC_COCKPIT_BUILT_AT || new Date().toISOString(),
  },
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
