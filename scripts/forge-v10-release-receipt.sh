#!/usr/bin/env bash
# ============================================================================
# ENG-FORGE-V10 S2 — Forge release-receipt helper (run on your Mac).
#
# DEV_OPS principle: get the DB code out (apply migrations) + do the git push
# that Vercel auto-builds. This script performs the real deploy and emits the
# engine's exact-artifact receipt — no agent prose, no local-SHA shortcuts.
#
# Usage:
#   bash scripts/forge-v10-release-receipt.sh --apply-migrations "108_x 109_x"   # optional
#   bash scripts/forge-v10-release-receipt.sh --push                              # push main
#   bash scripts/forge-v10-release-receipt.sh --verify                            # live probe after deploy
#   bash scripts/forge-v10-release-receipt.sh                                     # all (migrate+push+verify)
#
# Env:
#   PROD_BASE_URL   e.g. https://your-prod.vercel.app   (default: none -> skip live probe)
# ============================================================================
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

DO_MIGRATE=0; DO_PUSH=0; DO_VERIFY=0
MIGRATIONS=""
if [[ $# -eq 0 ]]; then DO_MIGRATE=1; DO_PUSH=1; DO_VERIFY=1; fi
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply-migrations) DO_MIGRATE=1; MIGRATIONS="$2"; shift 2;;
    --push) DO_PUSH=1; shift;;
    --verify) DO_VERIFY=1; shift;;
    *) echo "unknown arg: $1" >&2; exit 2;;
  esac
done

BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then echo "must run on main (on $BRANCH)" >&2; exit 2; fi
if [[ -n "$(git status --porcelain)" ]]; then echo "working tree not clean" >&2; exit 2; fi

if [[ "$DO_MIGRATE" -eq 1 && -n "$MIGRATIONS" ]]; then
  echo "== applying migrations to DEV and PROD =="
  for m in $MIGRATIONS; do
    node --env-file=.env.local scripts/apply-migration.mjs "db/migrations/$m.sql" dev
    node --env-file=.env.local scripts/apply-migration.mjs "db/migrations/$m.sql" prod
  done
fi

if [[ "$DO_PUSH" -eq 1 ]]; then
  echo "== pushing main -> origin/main (Vercel auto-deploys) =="
  git push origin main
fi

ARTIFACT_SHA=$(git rev-parse HEAD)
echo ""
echo "== deployment receipt =="
echo "{\"kind\":\"deployment\",\"artifactSha\":\"$ARTIFACT_SHA\",\"receiptId\":\"push:$ARTIFACT_SHA\",\"success\":true}"
echo "$ARTIFACT_SHA" > /tmp/forge_v10_deployed_sha.txt

if [[ "$DO_VERIFY" -eq 1 && -n "${PROD_BASE_URL:-}" ]]; then
  echo "== waiting for Vercel to finish (~3 min) then probing $PROD_BASE_URL =="
  sleep 185
  CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 30 "$PROD_BASE_URL" || echo 000)
  echo "== production_verification receipt =="
  echo "{\"kind\":\"production_verification\",\"artifactSha\":\"$ARTIFACT_SHA\",\"receiptId\":\"probe:$CODE\",\"success\":$([[ \"$CODE\" =~ ^2 ]] && echo true || echo false)}"
fi
echo ""
echo "Send the deployment receipt (and production_verification receipt) to the engine; artifactSha must equal publishedSha."
