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
  // BOTH BUNDLERS ARE DECLARED, and that is not decoration.
  //
  // Next 16 builds with Turbopack by default, and it REFUSES a project that has a `webpack` config and no
  // `turbopack` config ("This build is using Turbopack, with a `webpack` config and no `turbopack` config.
  // This may be a mistake."). That error broke the release build on 2026-09-15 the moment the edge fix
  // below added a `webpack` key — measured: `vercel build --prod` → `pnpm run build` → `next build`
  // (Turbopack) → build error, while `next build --webpack` (the command in AGENTS.md) was green.
  //
  // So: an explicit empty Turbopack config here, and the webpack config below applies to the webpack
  // path. Neither bundler is left to guess, and the release path keeps working with the Turbopack default.
  turbopack: {},
  // EDGE-SAFE INSTRUMENTATION (measured 2026-09-15, found by running `pnpm exec next build`).
  //
  // `instrumentation.ts` is compiled for BOTH runtimes. Its database write is already guarded at
  // runtime (`if (process.env.NEXT_RUNTIME !== 'nodejs') return`), but the guard does not stop
  // WEBPACK: the edge compilation still walks the dynamically imported graph, reaches
  // `db/client.ts` → `pg` → `require('fs')` / `require('path')` / `require('stream')`, and the build
  // fails with "Module not found: Can't resolve 'fs'" plus an import trace ending at
  // `./instrumentation.ts`. Vercel rejected the same module graph at deploy time as
  // "The Edge Function 'middleware' is referencing unsupported modules" — so this is a release
  // blocker, not a warning.
  //
  // The fix is to satisfy the bundler for code that can never execute on Edge: the Node builtins
  // below resolve to EMPTY modules in the edge compilation only. Both `fs` and `node:fs` forms are
  // listed because webpack treats them as different requests — measured: stubbing `fs` alone moved the
  // failure to `node:fs` via `lib/execution-target.ts`. `serverExternalPackages` above already keeps
  // `pg` unbundled for the real (Node) server, so nothing about runtime behaviour changes.
  //
  // The set is deliberately broad. On the Edge runtime NONE of these modules exist, so a bundle that
  // could resolve them would fail at runtime anyway; what this prevents is webpack failing the BUILD
  // while inspecting a graph that a `NEXT_RUNTIME` guard has already excluded. The one thing it does
  // not protect against is new edge code importing the database unguarded — that would now build and
  // fail at runtime, so keep the guard in `instrumentation.ts` in place.
  webpack: (config, { nextRuntime }) => {
    if (nextRuntime === 'edge') {
      const stubbed = [
        'fs',
        'path',
        'stream',
        'util',
        'net',
        'tls',
        'dns',
        'os',
        'perf_hooks',
        'crypto',
        'buffer',
        'events',
        'child_process',
        'cluster',
        'dgram',
        'http',
        'https',
        'module',
        'querystring',
        'readline',
        'repl',
        'string_decoder',
        'timers',
        'url',
        'v8',
        'vm',
        'worker_threads',
        'zlib',
        'assert',
      ]
      config.resolve = config.resolve ?? {}
      config.resolve.fallback = {
        ...(config.resolve.fallback ?? {}),
        ...Object.fromEntries(stubbed.map((name) => [name, false])),
      }
      // The `node:`-prefixed forms need ALIASES, not fallbacks: webpack resolves a `node:`-scheme
      // request before consulting `resolve.fallback`, which is why stubbing `node:fs` there changed
      // nothing and the build failed on the same import. Measured twice, so it is written down.
      config.resolve.alias = {
        ...(config.resolve.alias ?? {}),
        ...Object.fromEntries(stubbed.map((name) => [`node:${name}`, false])),
      }
    }
    return config
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
