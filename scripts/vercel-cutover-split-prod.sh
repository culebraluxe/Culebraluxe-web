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
  "framework": "container",
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

  if ! node --env-file-if-exists="$ROOT_DIR/.env.local" - \
    "$TMP_DIR/env-value.json" "$TMP_DIR/env-upsert.json" "$COPIED_KEYS" "$TMP_DIR/bridge-source.json" <<'NODE'
const fs = require('fs')
const inputPath = process.argv[2]
const outputPath = process.argv[3]
const copiedPath = process.argv[4]
const bridgePath = process.argv[5]
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
if (entry.key === 'CULEBRA_INTERNAL_API_KEY' || entry.key === 'AUTH_SECRET') {
  let bridge = {}
  try { bridge = JSON.parse(fs.readFileSync(bridgePath, 'utf8')) } catch {}
  bridge[entry.key] = entry.value
  fs.writeFileSync(bridgePath, JSON.stringify(bridge), { mode: 0o600 })
}
NODE
  then
    continue
  fi

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
  if [[ "$KEY" == "CULEBRA_INTERNAL_API_KEY" || "$KEY" == "AUTH_SECRET" ]]; then
    node - "$FALLBACK" "$TMP_DIR/bridge-source.json" <<'NODE'
const fs = require('fs')
const source = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const targetPath = process.argv[3]
let target = {}
try { target = JSON.parse(fs.readFileSync(targetPath, 'utf8')) } catch {}
target[source.key] = source.value
fs.writeFileSync(targetPath, JSON.stringify(target), { mode: 0o600 })
NODE
  fi
done

grep -qx 'DATABASE_URL_PROD' "$COPIED_KEYS" \
  || fail "DATABASE_URL_PROD could not be copied from Vercel or .env.local."
if ! grep -qx 'CULEBRA_INTERNAL_API_KEY' "$COPIED_KEYS" && ! grep -qx 'AUTH_SECRET' "$COPIED_KEYS"; then
  fail "Neither CULEBRA_INTERNAL_API_KEY nor AUTH_SECRET could be copied from Vercel or .env.local."
fi
printf '  production environment copied to Rust project\n'

# Pin Rust to the FRONTEND'S EFFECTIVE bridge key. The frontend uses an explicit
# CULEBRA_INTERNAL_API_KEY when present; otherwise it derives one from AUTH_SECRET.
# A stale explicit key on the standalone Rust project overrides AUTH_SECRET and causes
# every real portal request to 401 even though /readyz still passes.
node --env-file-if-exists="$ROOT_DIR/.env.local" - \
  "$TMP_DIR/bridge-source.json" "$TMP_DIR/bridge-key.json" <<'NODE'
const fs = require('fs')
const crypto = require('crypto')
const sourcePath = process.argv[2]
const outputPath = process.argv[3]
let source = {}
try { source = JSON.parse(fs.readFileSync(sourcePath, 'utf8')) } catch {}
const explicit = String(source.CULEBRA_INTERNAL_API_KEY ?? process.env.CULEBRA_INTERNAL_API_KEY ?? '').trim()
let value = explicit.length >= 16 ? explicit : ''
if (!value) {
  const authSecret = String(source.AUTH_SECRET ?? process.env.AUTH_SECRET ?? '').trim()
  if (authSecret.length < 16) {
    console.error('Cannot resolve the frontend bridge key: neither CULEBRA_INTERNAL_API_KEY nor AUTH_SECRET is available.')
    process.exit(12)
  }
  value = crypto.createHash('sha256')
    .update('culebraluxe-rust-bridge:v1:')
    .update(authSecret)
    .digest('hex')
}
fs.writeFileSync(outputPath, JSON.stringify({
  key: 'CULEBRA_INTERNAL_API_KEY',
  value,
  type: 'encrypted',
  target: ['production'],
  comment: 'Effective frontend-to-Rust bridge key',
}), { mode: 0o600 })
NODE
vc api "/v10/projects/${RUST_PROJECT_ID}/env?upsert=true&teamId=${TEAM_ID}" \
  -X POST --input "$TMP_DIR/bridge-key.json" >/dev/null
printf '  Rust bridge key synchronized to frontend effective key\n'

# Production pool settings are operational configuration, not secrets. Pin them here so an
# older value on the source frontend project cannot silently override the Rust service defaults.
for POOL_SETTING in \
  'FORGE_DB_POOL_MAX=12' \
  'FORGE_DB_POOL_MIN=5' \
  'FORGE_DB_POOL_CONNECT_MS=15000'
do
  KEY="${POOL_SETTING%%=*}"
  VALUE="${POOL_SETTING#*=}"
  node - "$KEY" "$VALUE" "$TMP_DIR/pool-setting.json" <<'NODE'
const fs = require('fs')
const key = process.argv[2]
const value = process.argv[3]
const outputPath = process.argv[4]
fs.writeFileSync(outputPath, JSON.stringify({
  key,
  value,
  type: 'plain',
  target: ['production'],
  comment: 'Rust production database pool sizing',
}))
NODE
  vc api "/v10/projects/${RUST_PROJECT_ID}/env?upsert=true&teamId=${TEAM_ID}" \
    -X POST --input "$TMP_DIR/pool-setting.json" >/dev/null
done
printf '  Rust PROD pool pinned: min=5 max=12 connect=15s\n'

# The Rust API has its own internal-key authentication on every application route.
# Production needs to be reachable from the Next frontend without Vercel login interposition.
cat >"$TMP_DIR/rust-project-public.json" <<'JSON'
{
  "ssoProtection": null,
  "passwordProtection": null,
  "trustedIps": null
}
JSON
vc api "/v9/projects/${RUST_PROJECT_ID}?teamId=${TEAM_ID}" -X PATCH --input "$TMP_DIR/rust-project-public.json" >/dev/null
# Belt-and-suspenders: use the current CLI protection command too. Ignore individual
# toggles that are already disabled or unavailable on the current plan.
vc project protection disable "$RUST_PROJECT_NAME" --sso >/dev/null 2>&1 || true
vc project protection disable "$RUST_PROJECT_NAME" --password >/dev/null 2>&1 || true

printf 'Configuring standalone Rust project with the Vercel Container preset...\n'
vc project update "$RUST_PROJECT_NAME" --framework container --scope "$TEAM_SLUG" >/dev/null

printf 'Validating Vercel sees rust/Dockerfile.vercel as a container deployment...\n'
set +e
RUST_DRY_RUN="$(
  cd "$ROOT_DIR/rust"
  VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$RUST_PROJECT_ID" vc deploy --dry 2>&1
)"
RUST_DRY_STATUS=$?
set -e
printf '%s\n' "$RUST_DRY_RUN"
[[ "$RUST_DRY_STATUS" -eq 0 ]] || fail "Vercel dry-run failed for the Rust container project."
if ! printf '%s\n' "$RUST_DRY_RUN" | grep -Eiq 'container|Dockerfile\.vercel'; then
  fail "Vercel dry-run did not identify the Rust project as a container/Dockerfile deployment."
fi

printf 'Deploying Rust API from rust/Dockerfile.vercel...\n'
RUST_DEPLOY_OUTPUT="$(
  cd "$ROOT_DIR/rust"
  VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$RUST_PROJECT_ID" vc deploy --prod --yes 2>&1
)"
printf '%s\n' "$RUST_DEPLOY_OUTPUT"

if ! printf '%s\n' "$RUST_DEPLOY_OUTPUT" | grep -Eiq 'Dockerfile\.vercel|container|Building image'; then
  printf '\nWARNING: Vercel deploy output did not explicitly mention Dockerfile/container detection.\n' >&2
fi

RUST_DEPLOY_URL="$(printf '%s\n' "$RUST_DEPLOY_OUTPUT" | grep -Eo 'https://[A-Za-z0-9._-]+\.vercel\.app' | head -1 || true)"
[[ -n "$RUST_DEPLOY_URL" ]] || fail "Rust deployment completed without a deployment URL."

# Use the PROJECT'S STABLE PRODUCTION ALIAS for the application contract. Deployment-specific
# URLs can be protected independently and change on every deploy; the stable alias does neither.
RUST_PROD_URL="https://${RUST_PROJECT_NAME}.vercel.app"

printf 'Waiting for Rust readiness on PROD Neon at %s...\n' "$RUST_PROD_URL"
READY=""
HTTP_STATUS=""
for attempt in $(seq 1 12); do
  BODY_FILE="$TMP_DIR/ready-body"
  HTTP_STATUS="$(curl -sS -L --max-time 20 -o "$BODY_FILE" -w '%{http_code}' "${RUST_PROD_URL%/}/readyz" 2>"$TMP_DIR/ready-stderr" || true)"
  READY="$(cat "$BODY_FILE" 2>/dev/null || true)"
  if [[ "$HTTP_STATUS" == "200" ]] && printf '%s' "$READY" | grep -q '"ok":true' && printf '%s' "$READY" | grep -q '"databaseTarget":"prod"'; then
    break
  fi
  printf '  waiting for Rust API... (%s/12, HTTP %s)\n' "$attempt" "${HTTP_STATUS:-000}"
  sleep 5
done
if [[ "$HTTP_STATUS" != "200" ]] || ! printf '%s' "$READY" | grep -q '"ok":true'; then
  printf '\nRust readiness diagnostics:\n' >&2
  printf '  URL:    %s/readyz\n' "${RUST_PROD_URL%/}" >&2
  printf '  HTTP:   %s\n' "${HTTP_STATUS:-000}" >&2
  printf '  body:   %s\n' "${READY:-<empty>}" >&2
  printf '  curl:   %s\n' "$(cat "$TMP_DIR/ready-stderr" 2>/dev/null || true)" >&2
  printf '\nRecent Vercel logs:\n' >&2
  VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$RUST_PROJECT_ID" \
    vc logs "$RUST_PROD_URL" --since 15m 2>&1 | tail -80 >&2 || true
  fail "Rust API deployment is Ready in Vercel but its stable production alias is not healthy."
fi
printf '%s' "$READY" | grep -q '"databaseTarget":"prod"' || fail "Rust API is not pointed at PROD: $READY"
printf '  Rust ready: %s\n' "$READY"

printf 'Verifying frontend-to-Rust internal bridge authentication...\n'
BRIDGE_KEY="$(node -e "const j=require(process.argv[1]);process.stdout.write(j.value)" "$TMP_DIR/bridge-key.json")"
BRIDGE_BODY="$TMP_DIR/bridge-body"
BRIDGE_STATUS="$(curl -sS -L --max-time 20 -o "$BRIDGE_BODY" -w '%{http_code}' \
  -H "accept: application/json" \
  -H "x-culebra-internal-key: $BRIDGE_KEY" \
  "${RUST_PROD_URL%/}/v1/diagnostics/db" 2>"$TMP_DIR/bridge-stderr" || true)"
unset BRIDGE_KEY
if [[ "$BRIDGE_STATUS" != "200" ]]; then
  printf '  bridge HTTP: %s\n' "${BRIDGE_STATUS:-000}" >&2
  printf '  bridge body: %s\n' "$(cat "$BRIDGE_BODY" 2>/dev/null || true)" >&2
  fail "Rust is ready but the frontend internal bridge key is not accepted."
fi
printf '  internal bridge authentication accepted\n'

printf 'Pointing frontend production at %s...\n' "$RUST_PROD_URL"
node - "$RUST_PROD_URL" "$TMP_DIR/frontend-rust-url.json" <<'NODE'
const fs = require('fs')
const value = process.argv[2]
const outputPath = process.argv[3]
fs.writeFileSync(outputPath, JSON.stringify({
  key: 'RUST_API_BASE_URL',
  value,
  type: 'plain',
  target: ['production'],
  comment: 'Standalone Rust API production deployment',
}))
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
