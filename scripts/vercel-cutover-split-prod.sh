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
vc api "/v10/projects/${FRONTEND_PROJECT_ID}/env?teamId=${TEAM_ID}" >"$TMP_DIR/frontend-env-list.json"

node - "$TMP_DIR/frontend-env-list.json" "$TMP_DIR/frontend-env-ids.txt" <<'NODE'
const fs = require('fs')
const inputPath = process.argv[2]
const outputPath = process.argv[3]
const body = JSON.parse(fs.readFileSync(inputPath, 'utf8'))
const envs = Array.isArray(body.envs) ? body.envs : []
const ids = []
for (const entry of envs) {
  if (!entry || !entry.id || !entry.key) continue
  const targets = Array.isArray(entry.target) ? entry.target : [entry.target]
  if (!targets.includes('production')) continue
  if (entry.system === true || entry.system === 'true') continue
  if (entry.key.startsWith('VERCEL_')) continue
  if (entry.key === 'RUST_API_BASE_URL') continue
  ids.push(entry.id)
}
fs.writeFileSync(outputPath, ids.join('\n') + (ids.length ? '\n' : ''))
NODE

COPIED_KEYS="$TMP_DIR/copied-keys.txt"
: >"$COPIED_KEYS"

while IFS= read -r ENV_ID; do
  [[ -n "$ENV_ID" ]] || continue

  # Vercel deprecated bulk ?decrypt=true. Retrieve each value through the supported
  # per-variable endpoint instead. Output stays in the mode-700 temp directory.
  if ! vc api "/v1/projects/${FRONTEND_PROJECT_ID}/env/${ENV_ID}?teamId=${TEAM_ID}" >"$TMP_DIR/env-value.json" 2>/dev/null; then
    continue
  fi

  node --env-file-if-exists="$ROOT_DIR/.env.local" - \
    "$TMP_DIR/env-value.json" "$TMP_DIR/env-upsert.json" "$COPIED_KEYS" <<'NODE'
const fs = require('fs')
const inputPath = process.argv[2]
const outputPath = process.argv[3]
const copiedPath = process.argv[4]
const raw = JSON.parse(fs.readFileSync(inputPath, 'utf8'))
const entry = raw.env ?? raw
if (!entry || !entry.key || typeof entry.value !== 'string' || !entry.value.trim()) {
  process.exit(20)
}
const type = ['plain', 'encrypted', 'sensitive'].includes(entry.type) ? entry.type : 'encrypted'
fs.writeFileSync(outputPath, JSON.stringify({
  key: entry.key,
  value: entry.value,
  type,
  target: ['production'],
  ...(entry.comment ? { comment: entry.comment } : {}),
}))
fs.appendFileSync(copiedPath, entry.key + '\n')
NODE

  vc api "/v10/projects/${RUST_PROJECT_ID}/env?upsert=true&teamId=${TEAM_ID}" \
    -X POST --input "$TMP_DIR/env-upsert.json" >/dev/null
done <"$TMP_DIR/frontend-env-ids.txt"

# A locally-held secret can fill a gap if Vercel marks a production variable sensitive
# and refuses to return its value through the API. Only the Rust-required keys are eligible.
node --env-file-if-exists="$ROOT_DIR/.env.local" - "$COPIED_KEYS" "$TMP_DIR/local-fallbacks" <<'NODE'
const fs = require('fs')
const copiedPath = process.argv[2]
const outputDir = process.argv[3]
fs.mkdirSync(outputDir, { recursive: true, mode: 0o700 })
const copied = new Set(
  fs.existsSync(copiedPath)
    ? fs.readFileSync(copiedPath, 'utf8').split(/\r?\n/).filter(Boolean)
    : [],
)
const eligible = [
  'DATABASE_URL_PROD',
  'CULEBRA_INTERNAL_API_KEY',
  'AUTH_SECRET',
  'MUX_TOKEN_ID',
  'MUX_TOKEN_SECRET',
  'MUX_TOKEN_ID_PROD',
  'MUX_TOKEN_SECRET_PROD',
]
for (const key of eligible) {
  const value = process.env[key]
  if (copied.has(key) || typeof value !== 'string' || !value.trim()) continue
  fs.writeFileSync(
    `${outputDir}/${key}.json`,
    JSON.stringify({ key, value, type: 'encrypted', target: ['production'] }),
  )
}
NODE

for FALLBACK in "$TMP_DIR"/local-fallbacks/*.json; do
  [[ -f "$FALLBACK" ]] || continue
  KEY="$(node -e "const j=require(process.argv[1]);process.stdout.write(j.key)" "$FALLBACK")"
  vc api "/v10/projects/${RUST_PROJECT_ID}/env?upsert=true&teamId=${TEAM_ID}" \
    -X POST --input "$FALLBACK" >/dev/null
  printf '%s\n' "$KEY" >>"$COPIED_KEYS"
done

grep -qx 'DATABASE_URL_PROD' "$COPIED_KEYS" \
  || fail "DATABASE_URL_PROD could not be copied from Vercel or .env.local."
if ! grep -qx 'CULEBRA_INTERNAL_API_KEY' "$COPIED_KEYS" && ! grep -qx 'AUTH_SECRET' "$COPIED_KEYS"; then
  fail "Neither CULEBRA_INTERNAL_API_KEY nor AUTH_SECRET could be copied from Vercel or .env.local."
fi
printf '  production environment copied to Rust project\n'

# The Rust API has its own internal-key authentication on every application route.
# Production needs to be reachable from the Next frontend without Vercel login interposition.
cat >"$TMP_DIR/rust-project-public.json" <<'JSON'
{
  "ssoProtection": null
}
JSON
vc api "/v9/projects/${RUST_PROJECT_ID}?teamId=${TEAM_ID}" -X PATCH --input "$TMP_DIR/rust-project-public.json" >/dev/null

printf 'Deploying the already-built Rust image as a normal Vercel container project...\n'
set +e
RUST_DEPLOY_OUTPUT="$(
  cd "$ROOT_DIR/deploy/rust-api"
  VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$RUST_PROJECT_ID" vc deploy --prod --yes 2>&1
)"
RUST_DEPLOY_STATUS=$?
set -e
if [[ "$RUST_DEPLOY_STATUS" -ne 0 ]]; then
  printf '%s\n' "$RUST_DEPLOY_OUTPUT"
  printf 'Registry-image reuse was rejected; deploying the Rust source project directly instead...\n'
  RUST_DEPLOY_OUTPUT="$(
    cd "$ROOT_DIR/rust"
    VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$RUST_PROJECT_ID" vc deploy --prod --yes 2>&1
  )"
fi
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
