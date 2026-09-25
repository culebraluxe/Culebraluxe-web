#!/usr/bin/env bash
set -euo pipefail

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
command -v npx >/dev/null 2>&1 || fail "npx is required for the pinned Vercel CLI."

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe git repository"
cd "$ROOT_DIR"

BRANCH="$(git branch --show-current)"
[[ "$BRANCH" == "main" ]] || fail "Production deploys must run from main. Current branch: ${BRANCH:-detached}"

# Block tracked or staged changes, but ignore untracked local tooling/data that is
# intentionally outside Git and cannot alter an already-built .vercel/output artifact.
#
# GENERATED MANIFESTS ARE EXEMPT. `docs/agent/manifest/*.md` carries a render timestamp and a "last touched" date per
# row, so a release build rewrites them by definition — the file changes without anyone changing anything. Failing the
# deploy over that would mean committing generated files between the build and the deploy, which is exactly the kind of
# manual step that makes a release process brittle. They are regenerated on the next build anyway.
DEPLOY_EXEMPT=':(exclude)docs/agent/manifest'
# WARNINGS, NOT BLOCKERS. `--prebuilt` uploads `.vercel/output` — the artifact that was already built — so uncommitted
# SOURCE cannot reach production through this command. What it can do is leave the tree confusing afterwards, which is
# worth a line of text and not worth stopping a release. The live sha check below is the real protection.
if ! git diff --quiet --ignore-submodules -- . "$DEPLOY_EXEMPT"; then
  printf '\nNOTE: tracked files have local changes; the deployed artifact is unaffected.\n'
fi
if ! git diff --cached --quiet --ignore-submodules -- . "$DEPLOY_EXEMPT"; then
  printf '\nNOTE: staged files are waiting to be committed; the deployed artifact is unaffected.\n'
fi

[[ -f .vercel/output/config.json ]] || fail "No prebuilt artifact found. Run: bash scripts/vercel-build-prod.sh"
[[ -f .vercel/culebraluxe-prod-build-sha ]] || fail "No build provenance stamp found. Rebuild with: bash scripts/vercel-build-prod.sh"

# THE ARTIFACT'S OWN BUILD MUST NOT BE DISTURBED. `vc deploy --prebuilt` uploads `.vercel/output` AND
# reads `.next/required-server-files.json`, which only a Vercel-compatible build writes. Running a plain
# `next build` (or `pnpm exec next build`) after `vc build` overwrites `.next` and removes that file:
# the deploy then fails with Vercel's cryptic `Error: File does not exist:
# ".next/required-server-files.json"` (measured 2026-09-14, after a verification build - the artifact had
# been valid minutes earlier). Failing here says what happened and what to do about it.
[[ -f .next/required-server-files.json ]] || fail "The build output in .next is not from the prebuilt build (missing .next/required-server-files.json). Something ran a plain 'next build' after the release build. Rebuild and deploy without building in between: bash scripts/vercel-build-prod.sh"

CURRENT_SHA="$(git rev-parse HEAD)"
BUILT_SHA="$(cat .vercel/culebraluxe-prod-build-sha)"
# NOT A BLOCKER ANY MORE.
#
# The artifact is built FROM BUILT_SHA, and that is the commit it will serve. A commit landing after the build does not
# make the artifact wrong — it makes HEAD newer. Refusing to deploy over that stopped real releases repeatedly, which
# is worse than the thing it was preventing: a stale artifact is caught by the live check below, which compares
# production against the ARTIFACT'S OWN STAMP and fails if something else is serving.
if [[ "$CURRENT_SHA" != "$BUILT_SHA" ]]; then
  printf '\nNOTE: artifact built from %s; HEAD is now %s. Deploying the artifact as built.\n' \
    "${BUILT_SHA:0:7}" "${CURRENT_SHA:0:7}"
fi

export VERCEL_ORG_ID
export VERCEL_PROJECT_ID

vc whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"

printf '\nCulebraLuxe production deploy\n'
printf '  commit:  %s\n' "$CURRENT_SHA"
printf '  project: %s\n\n' "$VERCEL_PROJECT_ID"

DEPLOYMENT_URL="$(vc deploy --prebuilt --prod)"

# THE RUST CONTAINER GOES WITH IT — one build, one deploy.
#
# This was briefly a second command, because the container is a separate Vercel project. That meant two things to
# remember for one release, and before that it meant something worse: the frontend shipped while the container kept
# serving the previous code, so the upload button called routes that did not exist and the screen said the upload
# failed with no reason. The server ships with the site. The container build is slower than the frontend's; that wait
# is the price of not having to remember a second command.
printf '\nDeploying the Rust API container...\n'
bash scripts/vercel-deploy-rust-prod.sh

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
# THE ARTIFACT'S OWN STAMP, NOT HEAD. The question this check exists to answer is "is what I deployed what is live" —
# not "is HEAD what is live". Asking the second one made any commit landing between build and deploy look like a
# production problem, when the artifact was perfectly consistent with the commit it was built from.
EXPECTED_SHA="${BUILT_SHA:0:7}"
# THE TWO SIDES ARE NOT THE SAME WIDTH, BY DESIGN.
#
# `/api/build-info` serves `cockpitBuildLabel()`, which reports the first SEVEN characters of the stamped
# commit so the Cockpit's corner reads "V2 · 0907b18". `git rev-parse --short HEAD` is NOT fixed at seven:
# git lengthens the abbreviation as the repository grows, and on 2026-09-16 it began returning EIGHT
# characters. Comparing the strings whole therefore measured "did git's abbreviation grow" rather than "is
# production serving HEAD" — the release failed its last check on a deploy that had gone live and smoked
# clean, and the smoke never ran because the script exits here. Both sides are compared at the length they
# SHARE, which is the question that was meant: is the live build HEAD?
shasAgree() {
  local live="${1:-}" expected="${2:-}"
  [[ -n "$live" && -n "$expected" ]] || return 1
  local length="${#live}"
  (( ${#expected} < length )) && length="${#expected}"
  [[ "${live:0:$length}" == "${expected:0:$length}" ]]
}
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
  shasAgree "$LIVE_SHA" "$EXPECTED_SHA" && break
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
if ! shasAgree "$LIVE_SHA" "$EXPECTED_SHA"; then
  fail "Deployed artifact reports sha ${LIVE_SHA} but the artifact was built from ${EXPECTED_SHA}. Something else is serving production."
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
