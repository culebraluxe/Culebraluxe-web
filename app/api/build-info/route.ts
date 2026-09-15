import { COCKPIT_VERSION, cockpitBuildLabel, cockpitBuiltAt } from '@/lib/cockpit-version'

// PUBLIC, and deliberately so: this is how a deploy is VERIFIED.
//
// Production is built locally and deployed prebuilt (`scripts/vercel-deploy-prod.sh`), which means the
// question "did the artifact I just deployed actually go live?" has no answer unless the artifact can be
// asked. It carries no secret — a version, a short commit and a build time — and it is the same stamp the
// Cockpit's corner shows, so the corner, this endpoint and `git rev-parse HEAD` can be compared.
//
//   curl -s https://www.culebraluxe.com/api/build-info
export const dynamic = 'force-dynamic'

export async function GET() {
  return Response.json(
    {
      version: COCKPIT_VERSION,
      sha: cockpitBuildLabel(),
      builtAt: cockpitBuiltAt() || null,
    },
    { headers: { 'cache-control': 'no-store' } },
  )
}
