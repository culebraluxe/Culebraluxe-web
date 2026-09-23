#!/usr/bin/env bash
set -euo pipefail

TEAM_ID="team_xk8vFaeSyY6CuSkS3OK55tTc"
TEAM_SLUG="culebraluxe-4460s-projects"
FRONTEND_PROJECT_ID="prj_RHzXYauXOgIh2abiEsMiQJ1V3jlV"
RUST_PROJECT_NAME="culebraluxe-rust-api"
VERCEL_CLI_VERSION="59.25.4"

fail() {
  printf '\nERROR: %s\n' "$1" >&2
  exit 1
}

vc() {
  npx --yes "vercel@${VERCEL_CLI_VERSION}" "$@"
}

command -v git >/dev/null 2>&1 || fail "git is required"
command -v node >/dev/null 2>&1 || fail "node is required"
command -v npx >/dev/null 2>&1 || fail "npx is required"

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe repository"
cd "$ROOT_DIR"

[[ "$(git branch --show-current)" == "main" ]] || fail "Production cutover must run from main."
git diff --quiet --ignore-submodules -- || fail "Tracked files have local changes. Commit or discard them first."
git diff --cached --quiet --ignore-submodules -- || fail "Staged files are waiting to be committed."

vc whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated."

TMP_DIR="$(mktemp -d)"
chmod 700 "$TMP_DIR"
trap 'rm -rf "$TMP_DIR"' EXIT

printf '\nCulebraLuxe production cutover — standard frontend + standalone Rust API\n'
printf '  commit: %s\n' "$(git rev-parse --short HEAD)"
printf '  Vercel CLI: %s\n\n' "$VERCEL_CLI_VERSION"

printf 'Ensuring standalone Rust project exists...\n'
if vc api "/v9/projects/${RUST_PROJECT_NAME}?teamId=${TEAM_ID}" >"$TMP_DIR/rust-project.json" 2>/dev/null; then
  :
else
  cat >"$TMP_DIR/create-project.json" <<JSON
{
  "name": "${RUST_PROJECT_NAME}",
  "framework": null,
  "skipGitConnectDuringLink": true
}
JSON
  vc api "/v11/projects?teamId=${TEAM_ID}" -X POST --input "$TMP_DIR/create-project.json" >"$TMP_DIR/rust-project.json"
fi

RUST_PROJECT_ID="$(node -e "const f=require('fs');const j=JSON.parse(f.readFileSync(process.argv[1],'utf8'));const p=j.project||j;if(!p.id)process.exit(2);process.stdout.write(p.id)" "$TMP_DIR/rust-project.json")"   || fail "Could not resolve standalone Rust project id."

printf '  Rust project: %s (%s)\n' "$RUST_PROJECT_NAME" "$RUST_PROJECT_ID"

printf 'Cloning production environment to Rust project without printing secret values...\n'
vc api "/v10/projects/${FRONTEND_PROJECT_ID}/env?decrypt=true&source=vercel-cli:pull&teamId=${TEAM_ID}" >"$TMP_DIR/frontend-env.json"

node "$TMP_DIR/frontend-env.json" "$TMP_DIR/rust-env.json" <<'NODE'
const fs = require('fs')
const inputPath = process.argv[2]
const outputPath = process.argv[3]
const body = JSON.parse(fs.readFileSync(inputPath, 'utf8'))
const envs = Array.isArray(body.envs) ? body.envs : []
const production = envs.filter((entry) => {
  const targets = Array.isArray(entry.target) ? entry.target : [entry.target]
  return targets.includes('production')
})
const cloned = []
for (const entry of production) {
  if (!entry || !entry.key || typeof entry.value !== 'string') continue
  if (entry.value === '[SENSITIVE]') continue
  if (entry.system === true || entry.system === 'true') continue
  if (entry.key.startsWith('VERCEL_')) continue
  if (entry.key === 'RUST_API_BASE_URL') continue
  const type = ['plain', 'encrypted', 'sensitive'].includes(entry.type) ? entry.type : 'encrypted'
  cloned.push({
    key: entry.key,
    value: entry.value,
    type,
    target: ['production'],
    ...(entry.comment ? { comment: entry.comment } : {}),
  })
}
const byKey = new Map(cloned.map((entry) => [entry.key, entry]))
if (!byKey.has('DATABASE_URL_PROD')) {
  console.error('DATABASE_URL_PROD could not be decrypted from the existing production project.')
  process.exit(10)
}
if (!byKey.has('CULEBRA_INTERNAL_API_KEY') && !byKey.has('AUTH_SECRET')) {
  console.error('Neither CULEBRA_INTERNAL_API_KEY nor AUTH_SECRET could be decrypted from the existing production project.')
  process.exit(11)
}
fs.writeFileSync(outputPath, JSON.stringify(cloned))
NODE

vc api "/v10/projects/${RUST_PROJECT_ID}/env?upsert=true&teamId=${TEAM_ID}" -X POST --input "$TMP_DIR/rust-env.json" >/dev/null

# The Rust API has its own internal-key authentication on every application route.
# Production needs to be reachable from the Next frontend without Vercel login interposition.
cat >"$TMP_DIR/rust-project-public.json" <<'JSON'
{
  "ssoProtection": null
}
JSON
vc api "/v9/projects/${RUST_PROJECT_ID}?teamId=${TEAM_ID}" -X PATCH --input "$TMP_DIR/rust-project-public.json" >/dev/null

printf 'Deploying the already-built Rust image as a normal Vercel container project...\n'
RUST_DEPLOY_OUTPUT="$(
  cd "$ROOT_DIR/deploy/rust-api"
  VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$RUST_PROJECT_ID"     vc deploy --prod --yes 2>&1
)"
printf '%s\n' "$RUST_DEPLOY_OUTPUT"

RUST_DEPLOY_URL="$(printf '%s\n' "$RUST_DEPLOY_OUTPUT" | grep -Eo 'https://[A-Za-z0-9._-]+\.vercel\.app' | tail -1 || true)"
[[ -n "$RUST_DEPLOY_URL" ]] || fail "Rust deployment completed without a deployment URL."

printf 'Waiting for Rust readiness on PROD Neon...\n'
READY=""
for attempt in $(seq 1 18); do
  READY="$(curl -fsS -L --max-time 15 "${RUST_DEPLOY_URL%/}/readyz" 2>/dev/null || true)"
  if printf '%s' "$READY" | grep -q '"ok":true' && printf '%s' "$READY" | grep -q '"databaseTarget":"prod"'; then
    break
  fi
  printf '  waiting for Rust API... (%s/18)\n' "$attempt"
  sleep 5
done
printf '%s' "$READY" | grep -q '"ok":true' || fail "Rust API did not become ready: ${READY:-no response}"
printf '%s' "$READY" | grep -q '"databaseTarget":"prod"' || fail "Rust API is not pointed at PROD: $READY"
printf '  Rust ready: %s\n' "$READY"

printf 'Pointing frontend production at %s...\n' "$RUST_DEPLOY_URL"
node - "$RUST_DEPLOY_URL" "$TMP_DIR/frontend-rust-url.json" <<'NODE'
const fs = require('fs')
const value = process.argv[2]
const outputPath = process.argv[3]
fs.writeFileSync(outputPath, JSON.stringify([{
  key: 'RUST_API_BASE_URL',
  value,
  type: 'plain',
  target: ['production'],
  comment: 'Standalone Rust API production deployment',
}]))
NODE
vc api "/v10/projects/${FRONTEND_PROJECT_ID}/env?upsert=true&teamId=${TEAM_ID}" -X POST --input "$TMP_DIR/frontend-rust-url.json" >/dev/null

printf 'Restoring frontend project to the normal Next.js framework preset...\n'
cat >"$TMP_DIR/frontend-project.json" <<'JSON'
{
  "framework": "nextjs"
}
JSON
vc api "/v9/projects/${FRONTEND_PROJECT_ID}?teamId=${TEAM_ID}" -X PATCH --input "$TMP_DIR/frontend-project.json" >/dev/null

printf '\nBuilding standard frontend production artifact...\n'
RELEASE_FORGE_HARNESS=skip bash "$ROOT_DIR/scripts/vercel-build-prod.sh"

printf '\nDeploying standard frontend production artifact...\n'
bash "$ROOT_DIR/scripts/vercel-deploy-prod.sh"
