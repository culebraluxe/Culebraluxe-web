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

# THE ARTIFACT'S OWN BUILD MUST NOT BE DISTURBED. `vercel deploy --prebuilt` uploads `.vercel/output` AND
# reads `.next/required-server-files.json`, which only a Vercel-compatible build writes. Running a plain
# `next build` (or `pnpm exec next build`) after `vercel build` overwrites `.next` and removes that file:
# the deploy then fails with Vercel's cryptic `Error: File does not exist:
# ".next/required-server-files.json"` (measured 2026-09-14, after a verification build - the artifact had
# been valid minutes earlier). Failing here says what happened and what to do about it.
[[ -f .next/required-server-files.json ]] || fail "The build output in .next is not from the prebuilt build (missing .next/required-server-files.json). Something ran a plain 'next build' after the release build. Rebuild and deploy without building in between: bash scripts/vercel-build-prod.sh"

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
# script and compiled in by next.config) and serves it from /api/build-info. Comparing that against HEAD
# is the only check that catches "the deploy succeeded but the OLD artifact is serving" - which is exactly
# the failure the locally built path could introduce, and which no deploy output would reveal.
#
# WHICH URL: the CANONICAL domain, not the deployment URL. Vercel Authentication protects
# `*.vercel.app` deployment URLs (measured 2026-09-14: the deployment URL answered 302 to a login page
# while the production domain answered 200), so asking the deployment URL could only ever report
# "no answer" - a check that cannot pass is worse than no check.
PROD_URL="${CULEBRALUXE_PROD_URL:-https://www.culebraluxe.com}"
EXPECTED_SHA="$(git rev-parse --short HEAD)"
# WAIT FOR THE ALIAS. A deployment is created before the production domain points at it (measured
# 2026-09-14: the domain still served the previous sha seconds after a successful deploy, and was
# serving the new one inside a minute). Checking once would report a false failure; checking by hand
# would train everyone to ignore the check.
VERIFY_ATTEMPTS="${DEPLOY_VERIFY_ATTEMPTS:-15}"
VERIFY_SLEEP_SECONDS="${DEPLOY_VERIFY_SLEEP_SECONDS:-10}"
LIVE_SHA=""
for attempt in $(seq 1 "$VERIFY_ATTEMPTS"); do
  LIVE_SHA="$(curl -fsS -L --max-time 25 "${PROD_URL%/}/api/build-info" 2>/dev/null \
    | sed -n 's/.*"sha"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' || true)"
  [[ "$LIVE_SHA" == "$EXPECTED_SHA" ]] && break
  printf '  waiting for the production alias... (%s/%s, live=%s)\n' \
    "$attempt" "$VERIFY_ATTEMPTS" "${LIVE_SHA:-no answer}"
  sleep "$VERIFY_SLEEP_SECONDS"
done

printf '\nPRODUCTION DEPLOY COMPLETE\n'
printf '  deployment: %s\n' "$DEPLOYMENT_URL"
printf '  canonical:  %s\n' "$PROD_URL"
printf '  expected sha: %s\n' "$EXPECTED_SHA"
printf '  live sha:     %s\n' "${LIVE_SHA:-<no answer from /api/build-info>}"

if [[ -z "$LIVE_SHA" ]]; then
  fail "Deployed, but ${PROD_URL}/api/build-info did not answer - cannot confirm what is live."
fi
if [[ "$LIVE_SHA" != "$EXPECTED_SHA" ]]; then
  fail "Deployed artifact reports sha ${LIVE_SHA} but HEAD is ${EXPECTED_SHA}. Something else is serving production."
fi
printf '\nVERIFIED: production is serving %s.\n' "$LIVE_SHA"
printf 'In the Cockpit, the corner reads V2 · %s.\n' "$LIVE_SHA"

# AND THEN ASK THE SITE ITSELF. The sha check above proves the right artefact is aliased;
# it says nothing about whether the pages render. A deploy that aliases a build whose
# /buyers 500s would pass every check above and report VERIFIED.
# `--expect-head` makes the sha assertion part of the smoke too, so the two checks cannot
# drift apart; the smoke's page markers are strings the pages own, not byte counts.
printf '\nLIVE SMOKE (does production actually work)\n'
if ! (cd "$ROOT_DIR" && node --import tsx scripts/prod-smoke.ts --expect-head); then
  fail "Deployed and aliased, but the live smoke failed. Production is answering but not behaving - see the checks above."
fi
printf '\nRELEASE COMPLETE AND SMOKED.\n'
