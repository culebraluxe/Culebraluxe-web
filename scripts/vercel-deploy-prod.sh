#!/usr/bin/env bash
set -euo pipefail

VERCEL_ORG_ID="team_xk8vFaeSyY6CuSkS3OK55tTc"
VERCEL_PROJECT_ID="prj_RHzXYauXOgIh2abiEsMiQJ1V3jlV"

fail() {
  printf '\nERROR: %s\n' "$1" >&2
  exit 1
}

command -v git >/dev/null 2>&1 || fail "git is required"
command -v vercel >/dev/null 2>&1 || fail "Vercel CLI is required. Install it with: npm install --global vercel@latest"

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe git repository"
cd "$ROOT_DIR"

BRANCH="$(git branch --show-current)"
[[ "$BRANCH" == "main" ]] || fail "Production deploys must run from main. Current branch: ${BRANCH:-detached}"

# Block tracked or staged changes, but ignore untracked local tooling/data that is
# intentionally outside Git and cannot alter an already-built .vercel/output artifact.
git diff --quiet --ignore-submodules -- || fail "Tracked files have local changes. Commit or discard them before deploying production."
git diff --cached --quiet --ignore-submodules -- || fail "Staged files are waiting to be committed. Commit or unstage them before deploying production."

[[ -f .vercel/output/config.json ]] || fail "No prebuilt artifact found. Run: bash scripts/vercel-build-prod.sh"
[[ -f .vercel/culebraluxe-prod-build-sha ]] || fail "No build provenance stamp found. Rebuild with: bash scripts/vercel-build-prod.sh"

CURRENT_SHA="$(git rev-parse HEAD)"
BUILT_SHA="$(cat .vercel/culebraluxe-prod-build-sha)"
[[ "$CURRENT_SHA" == "$BUILT_SHA" ]] || fail "Prebuilt artifact belongs to a different commit. Rebuild before deploying."

export VERCEL_ORG_ID
export VERCEL_PROJECT_ID

vercel whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"

printf '\nCulebraLuxe production deploy\n'
printf '  commit:  %s\n' "$CURRENT_SHA"
printf '  project: %s\n\n' "$VERCEL_PROJECT_ID"

DEPLOYMENT_URL="$(vercel deploy --prebuilt --prod)"

# VERIFY WHAT WENT LIVE. The artifact stamps its own commit (`NEXT_PUBLIC_COCKPIT_SHA`, set by the build
# script) and serves it from /api/build-info. Comparing that against HEAD is the only check that catches
# "the deploy succeeded but the OLD artifact is serving" - which is exactly the failure the locally built
# path could introduce, and which no deploy output would reveal.
EXPECTED_SHA="$(git rev-parse --short HEAD)"
LIVE_SHA="$(curl -fsS --max-time 20 "${DEPLOYMENT_URL%/}/api/build-info" 2>/dev/null \
  | sed -n 's/.*"sha"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' || true)"

printf '\nPRODUCTION DEPLOY COMPLETE\n'
printf '%s\n' "$DEPLOYMENT_URL"
printf '  expected sha: %s\n' "$EXPECTED_SHA"
printf '  live sha:     %s\n' "${LIVE_SHA:-<no answer from /api/build-info>}"

if [[ -z "$LIVE_SHA" ]]; then
  fail "Deployed, but /api/build-info did not answer - cannot confirm what is live. Check the deployment URL above."
fi
if [[ "$LIVE_SHA" != "$EXPECTED_SHA" ]]; then
  fail "Deployed artifact reports sha ${LIVE_SHA} but HEAD is ${EXPECTED_SHA}. Something else is serving production."
fi
printf '\nVERIFIED: production is serving %s.\n' "$LIVE_SHA"
printf 'In the Cockpit, the corner reads V2 · %s.\n' "$LIVE_SHA"
