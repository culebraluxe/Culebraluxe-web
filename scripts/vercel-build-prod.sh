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
command -v docker >/dev/null 2>&1 || fail "Docker is required for the Rust container service"
docker info >/dev/null 2>&1 || fail "Docker is installed but not running. Start Docker Desktop before the production build."

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe git repository"
cd "$ROOT_DIR"

NODE_MAJOR="$(node -p 'process.versions.node.split(".")[0]')"
if [[ "$NODE_MAJOR" != "$EXPECTED_NODE_MAJOR" ]]; then
  fail "Production builds require Node ${EXPECTED_NODE_MAJOR}. Current: $(node --version)."
fi

export VERCEL_ORG_ID
export VERCEL_PROJECT_ID
# Apple Silicon is the normal CulebraLuxe development host. Pin the container build target so the
# first production image is a Linux amd64 artifact rather than whatever architecture Docker Desktop
# happens to inherit from the host.
export DOCKER_DEFAULT_PLATFORM="${DOCKER_DEFAULT_PLATFORM:-linux/amd64}"

printf '\nCulebraLuxe local production build\n'
printf '  commit:  %s\n' "$(git rev-parse --short HEAD)"
printf '  node:    %s\n' "$(node --version)"
printf '  docker:  %s\n' "$DOCKER_DEFAULT_PLATFORM"
printf '  project: %s\n\n' "$VERCEL_PROJECT_ID"

vercel whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"

printf 'Pulling Vercel production settings...\n'
vercel pull --yes --environment=production

ENV_FILE=".vercel/.env.production.local"
[[ -f "$ENV_FILE" ]] || fail "Vercel production env file was not pulled: $ENV_FILE"
grep -q '^DATABASE_URL_PROD=' "$ENV_FILE" || fail "DATABASE_URL_PROD is missing from Vercel production environment"
if ! grep -q '^CULEBRA_INTERNAL_API_KEY=' "$ENV_FILE" && ! grep -q '^AUTH_SECRET=' "$ENV_FILE"; then
  fail "Rust bridge auth is missing: set CULEBRA_INTERNAL_API_KEY or AUTH_SECRET in Vercel production"
fi

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
  pnpm forge:harness
fi

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

printf '\nVerifying multi-service build output...\n'
node <<'NODE'
const fs = require('node:fs')
const config = JSON.parse(fs.readFileSync('.vercel/output/config.json', 'utf8'))
const raw = config.services
const services = Array.isArray(raw)
  ? raw
  : raw && typeof raw === 'object'
    ? Object.entries(raw).map(([name, value]) => ({ name, ...(value || {}) }))
    : []
const names = new Set(services.map((service) => service && service.name).filter(Boolean))
for (const required of ['frontend', 'rust_api']) {
  if (!names.has(required)) {
    console.error(`ERROR: prebuilt output is missing required service "${required}". Found: ${[...names].join(', ') || '<none>'}`)
    process.exit(1)
  }
}
const frontend = services.find((service) => service && service.name === 'frontend')
const bindings = Array.isArray(frontend?.bindings) ? frontend.bindings : []
const rustBinding = bindings.some(
  (binding) =>
    binding &&
    binding.type === 'service' &&
    binding.service === 'rust_api' &&
    binding.env === 'RUST_API_BASE_URL',
)
if (!rustBinding) {
  console.error('ERROR: frontend prebuilt output is missing the rust_api -> RUST_API_BASE_URL service binding.')
  process.exit(1)
}
console.log('  services: frontend + rust_api')
console.log('  binding:  rust_api -> RUST_API_BASE_URL')
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

# TIDY THE INTERMEDIATE TREE. `vercel build` leaves macOS copy-on-conflict duplicates behind — measured
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
