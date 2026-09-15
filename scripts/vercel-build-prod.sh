#!/usr/bin/env bash
set -euo pipefail

EXPECTED_NODE_MAJOR="24"
VERCEL_ORG_ID="team_xk8vFaeSyY6CuSkS3OK55tTc"
VERCEL_PROJECT_ID="prj_RHzXYauXOgIh2abiEsMiQJ1V3jlV"

fail() {
  printf '\nERROR: %s\n' "$1" >&2
  exit 1
}

command -v git >/dev/null 2>&1 || fail "git is required"
command -v node >/dev/null 2>&1 || fail "Node.js is required"
command -v vercel >/dev/null 2>&1 || fail "Vercel CLI is required. Install it with: npm install --global vercel@latest"

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe git repository"
cd "$ROOT_DIR"

NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
if [[ "$NODE_MAJOR" != "$EXPECTED_NODE_MAJOR" ]]; then
  fail "Production builds require Node ${EXPECTED_NODE_MAJOR}. Current: $(node --version)."
fi

export VERCEL_ORG_ID
export VERCEL_PROJECT_ID

printf '\nCulebraLuxe local production build\n'
printf '  commit:  %s\n' "$(git rev-parse --short HEAD)"
printf '  node:    %s\n' "$(node --version)"
printf '  project: %s\n\n' "$VERCEL_PROJECT_ID"

vercel whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"

printf 'Pulling Vercel production settings...\n'
vercel pull --yes --environment=production

printf '\nClearing previous prebuilt output...\n'
rm -rf .vercel/output

# THE HARNESS GATES THE RELEASE. This is the last automatic checkpoint before production, so it is where a
# drifted manifest, a hand-edited vendor block, or a packet citing a path that no longer exists should stop
# the build rather than ship. Deliberately not a commit hook: the repo has none, and this is the moment the
# checks are worth the wait. `set -e` above means a harness failure aborts the release here.
printf '\nRunning the harness gates (packet lint, manifests, vendor blocks)...\n'
pnpm forge:harness

# START THE BUILD FROM A CLEAN .next. `vercel build` builds ON TOP of whatever tree is already there, and
# repeated release builds had left 2,035 duplicate generated files (`cache-life.d 3.ts` and friends - the
# macOS copy-on-conflict rename), which is what broke `tsc` and `next build` until someone pruned by hand.
# Deleting the tree first is the source fix; pruning the duplicates afterwards is not.
printf '\nClearing the previous build output (.next)...\n'
rm -rf .next

printf '\nBuilding production artifact locally...\n'
# STAMP THE ARTIFACT WITH ITS OWN SOURCE. A local prebuilt deploy may have no Vercel git variables at
# all, and the Cockpit's corner plus /api/build-info are how a deploy is verified - so the commit and
# build time go into the bundle at build time (NEXT_PUBLIC_* is inlined) rather than being asked of the
# platform afterwards.
export NEXT_PUBLIC_COCKPIT_SHA="$(git rev-parse --short HEAD)"
export NEXT_PUBLIC_COCKPIT_BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf '  stamped: %s at %s\n' "$NEXT_PUBLIC_COCKPIT_SHA" "$NEXT_PUBLIC_COCKPIT_BUILT_AT"
vercel build --prod

[[ -f .vercel/output/config.json ]] || fail "Build completed without .vercel/output/config.json"

printf '\nRunning artifact safety checks...\n'
if grep -R -I -l -F '[SENSITIVE]' .vercel/output >/dev/null 2>&1; then
  fail "A Vercel [SENSITIVE] placeholder was embedded in .vercel/output. Do not deploy this artifact."
fi

PRIVATE_PATH_MATCH="$({
  find .vercel/output \
    \( -path '*/contact-export/*' \
       -o -path '*/apple-messages-output/*' \
       -o -path '*/apple-messages-export/output/*' \
       -o -path '*/public/upload/data/apple-messages-export/*' \
       -o -name 'culebraluxe-calendar*.json' \) \
    -print -quit
} 2>/dev/null || true)"
if [[ -n "$PRIVATE_PATH_MATCH" ]]; then
  fail "Private local data was traced into the prebuilt artifact: $PRIVATE_PATH_MATCH"
fi

git rev-parse HEAD > .vercel/culebraluxe-prod-build-sha

printf '\nLOCAL BUILD COMPLETE\n'
printf 'Artifact: .vercel/output\n'
printf 'Size:     %s\n' "$(du -sh .vercel/output | awk '{print $1}')"
printf 'Commit:   %s\n' "$(git rev-parse HEAD)"
printf 'Nothing has been deployed.\n'
