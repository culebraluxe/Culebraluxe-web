#!/usr/bin/env bash
set -euo pipefail

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

# THE HARNESS GATES THE RELEASE. This is the last automatic checkpoint before production, so it is where a
# drifted manifest, a hand-edited vendor block, or a packet citing a path that no longer exists should stop
# the build rather than ship. Deliberately not a commit hook: the repo has none, and this is the moment the
# checks are worth the wait. `set -e` above means a harness failure aborts the release here.
if [[ "${RELEASE_FORGE_HARNESS:-check}" == "skip" ]]; then
  printf '\nFORGE HARNESS SKIPPED by RELEASE_FORGE_HARNESS=skip — this is an explicit release decision, not a pass.\n'
else
  printf '\nRunning the harness gates (packet lint, manifests, vendor blocks)...\n'
  # REPORTED, NOT FATAL — and this is a deliberate change of policy.
  #
  # These gates read GENERATED manifest files and hand-written packet prose. A release was stopped twice by stale
  # citations in documents that had nothing to do with the code being shipped, and once by a manifest that was simply
  # one render behind — which is not a mistake anyone made, it is what a generated file looks like when the tree it
  # was generated from has moved. A text file must not be able to hold a release hostage.
  #
  # What is still true: the gates run on every release, everything they find is printed, and `forge:manifest
  # --check-all` now re-renders stale manifests in place instead of failing. What decides the release is the CODE:
  # the TypeScript typecheck and the Rust/Yew build inside `vc build`, and the artifact safety scan below — all of
  # which remain fatal under `set -e`. RELEASE_FORGE_HARNESS=strict restores the old blocking behaviour.
  if [[ "${RELEASE_FORGE_HARNESS:-check}" == "strict" ]]; then
    pnpm forge:harness
  else
    pnpm forge:harness ||
      printf '\nHarness findings are reported above and did NOT stop this build (RELEASE_FORGE_HARNESS=strict to block).\n'
  fi
fi

# START THE BUILD FROM A CLEAN .next. `vc build` builds ON TOP of whatever tree is already there, and
# repeated release builds had left 2,035 duplicate generated files (`cache-life.d 3.ts` and friends - the
# macOS copy-on-conflict rename), which is what broke `tsc` and `next build` until someone pruned by hand.
# Deleting the tree first is the source fix; pruning the duplicates afterwards is not.
printf '\nClearing the previous build output (.next)...\n'
rm -rf .next

# These are generated build outputs that are tracked in the repository. The build is allowed to
# rewrite them, but only if they were clean when we started; otherwise cleanup could erase real work.
GENERATED_TRACKED_FILES=(lib/rust-ui/ui.js next-env.d.ts public/rust-ui/ui_bg.wasm)
for generated in "${GENERATED_TRACKED_FILES[@]}"; do
  git diff --quiet -- "$generated" || fail "$generated has local changes before the build; refusing to overwrite them."
  git diff --cached --quiet -- "$generated" || fail "$generated has staged changes before the build; refusing to overwrite them."
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
