#!/usr/bin/env bash
set -euo pipefail

# SUPERSEDED 2026-10-03 — DO NOT RUN THIS TO RELEASE.
#
# This is the umbrella: it calls `scripts/vercel-build-prod.sh` and `scripts/vercel-deploy-prod.sh`, both of which
# belong to the older Vercel-prebuilt path and neither of which can complete for the application as it stands. The
# live release path is `pnpm deploy:prod` -> `scripts/deploy-prod.sh`. See `scripts/vercel-build-prod.sh`.

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "ERROR: Run this inside the CulebraLuxe git repository" >&2
  exit 1
}
cd "$ROOT_DIR"

BRANCH="$(git branch --show-current)"
if [[ "$BRANCH" != "main" ]]; then
  echo "ERROR: Production releases must run from main. Current branch: ${BRANCH:-detached}" >&2
  exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
  echo "ERROR: Working tree is not clean. Commit or discard local changes before releasing production." >&2
  exit 1
fi

printf '\n=== CulebraLuxe local production release ===\n'
printf 'Source commit: %s\n\n' "$(git rev-parse HEAD)"

bash scripts/vercel-build-prod.sh
bash scripts/vercel-deploy-prod.sh
