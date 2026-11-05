#!/usr/bin/env bash
set -euo pipefail

# SUPERSEDED 2026-10-03 — DO NOT RUN THIS TO RELEASE.
#
# The release path is `pnpm deploy:prod` -> `scripts/deploy-prod.sh`: ONE Vercel project, with the whole
# application compiled on this Mac inside `devops/Dockerfile.build` and only unpacked by Vercel
# (`devops/Dockerfile.runtime`).
#
# This file belongs to the older Vercel-prebuilt path (2026-09-25, "one build, one deploy"). It generates
# `lib/rust-ui/ui.js` — deleted with `lib/` in the TypeScript retirement — into a Next-era `.vercel/output` that no
# longer describes a Rust site in a container. Kept for the record, not for the release.
# `docs/agent/DEV-OPS-RELEASE.md` still lists this pair under "Kept for build-only and deploy-only work"; that list
# is older doctrine than `deploy:prod`, and this file cannot complete as it stands.

EXPECTED_NODE_MAJOR="24"
VERCEL_ORG_ID="team_xk8vFaeSyY6CuSkS3OK55tTc"
VERCEL_PROJECT_ID="prj_RHzXYauXOgIh2abiEsMiQJ1V3jlV"
VERCEL_CLI_VERSION="59.25.4"

vc() {
  npx --yes "vercel@${VERCEL_CLI_VERSION}" "$@"
}

fail() {
  printf '\nERROR: %s\n' "$1" >&2
  exit 1
}

command -v git >/dev/null 2>&1 || fail "git is required"
command -v node >/dev/null 2>&1 || fail "Node.js is required"
command -v npx >/dev/null 2>&1 || fail "npx is required for the pinned Vercel CLI."
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

vc whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"

printf 'Pulling Vercel production settings...\n'
vc pull --yes --environment=production

ENV_FILE=".vercel/.env.production.local"
rm -f "$ENV_FILE"
vc env pull "$ENV_FILE" --environment=production
[[ -f "$ENV_FILE" ]] || fail "Vercel production env file was not pulled: $ENV_FILE"
printf '\nClearing previous prebuilt output...\n'
rm -rf .vercel/output

# Forge harness gates moved to the Forge repo (X-6a); app release runs app gates only.

# START THE BUILD FROM A CLEAN .next. `vc build` builds ON TOP of whatever tree is already there, and
# repeated release builds had left 2,035 duplicate generated files (`cache-life.d 3.ts` and friends - the
# macOS copy-on-conflict rename), which is what broke `tsc` and `next build` until someone pruned by hand.
# Deleting the tree first is the source fix; pruning the duplicates afterwards is not.
printf '\nClearing the previous build output (.next)...\n'
rm -rf .next

# These are generated build outputs that are tracked in the repository. The build is allowed to
# rewrite them, but only if they were clean when we started; otherwise cleanup could erase real work.
GENERATED_TRACKED_FILES=(lib/rust-ui/ui.js next-env.d.ts public/rust-ui/ui_bg.wasm)
# A NOTE, NOT A BLOCKER. These three are generated build outputs that are tracked in Git, and this script REGENERATES
# them and then restores them (see `git restore` below), so their local state cannot be lost by building — and a build
# that ran and was interrupted leaves them dirty, which made the NEXT build refuse to start. That is the same mistake
# as refusing a deploy because the code moved on: it stops work over a condition the work itself creates.
for generated in "${GENERATED_TRACKED_FILES[@]}"; do
  if ! git diff --quiet -- "$generated" 2>/dev/null || ! git diff --cached --quiet -- "$generated" 2>/dev/null; then
    printf 'NOTE: %s has local changes; the build regenerates and restores it.\n' "$generated"
  fi
done

printf '\nBuilding production artifact locally...\n'
# STAMP THE ARTIFACT WITH ITS OWN SOURCE. A local prebuilt deploy may have no Vercel git variables at
# all, and the Cockpit's corner plus /api/build-info are how a deploy is verified - so the commit and
# build time go into the bundle at build time (NEXT_PUBLIC_* is inlined) rather than being asked of the
# platform afterwards.
export NEXT_PUBLIC_COCKPIT_SHA="$(git rev-parse --short HEAD)"
export NEXT_PUBLIC_COCKPIT_BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
printf '  stamped: %s at %s\n' "$NEXT_PUBLIC_COCKPIT_SHA" "$NEXT_PUBLIC_COCKPIT_BUILT_AT"
vc build --prod

[[ -f .vercel/output/config.json ]] || fail "Build completed without .vercel/output/config.json"

printf '\nVerifying standard frontend build output...\n'
node <<'NODE'
const fs = require('node:fs')
const config = JSON.parse(fs.readFileSync('.vercel/output/config.json', 'utf8'))
if (Array.isArray(config.services) && config.services.length > 0) {
  console.error('ERROR: frontend artifact unexpectedly contains Vercel Services output.')
  process.exit(1)
}
if (config.experimentalServicesV2) {
  console.error('ERROR: frontend artifact unexpectedly contains experimentalServicesV2.')
  process.exit(1)
}
console.log('  standard Next frontend artifact')
NODE


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

# Return tracked generated outputs to HEAD after the artifact is complete. The deployable bytes live in
# .vercel/output; restoring these source-tree copies does not alter the finished artifact.
git restore -- "${GENERATED_TRACKED_FILES[@]}"

# TIDY THE INTERMEDIATE TREE. `vc build` leaves macOS copy-on-conflict duplicates behind — measured
# 2026-09-16: 740 files named "cache-life.d 2.ts", "routes.d 2.ts" and so on, produced DURING the build even
# though this script deletes `.next` first, so clearing beforehand does not prevent them. They sit inside
# `.next/types`, which `tsc` includes, so the next typecheck fails with duplicate-identifier errors until
# someone prunes them. The deployable artifact is `.vercel/output` and is untouched by this.
printf '\nRemoving duplicate generated files from the intermediate tree...\n'
DUPLICATES="$(find .next -name '* [0-9].*' -type f 2>/dev/null | wc -l | tr -d ' ')"
if [[ "$DUPLICATES" != "0" ]]; then
  find .next -name '* [0-9].*' -type f -delete 2>/dev/null || true
  printf '  removed %s duplicate file(s)\n' "$DUPLICATES"
else
  printf '  none found\n'
fi

printf '\nLOCAL BUILD COMPLETE\n'
printf 'Artifact: .vercel/output\n'
printf 'Size:     %s\n' "$(du -sh .vercel/output | awk '{print $1}')"
printf 'Commit:   %s\n' "$(git rev-parse HEAD)"
printf 'Nothing has been deployed.\n'
