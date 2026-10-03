#!/usr/bin/env bash
# PRODUCTION DEPLOY — the CulebraLuxe application to its one Vercel project, culebraluxe-web-fp, which serves
# culebraluxe.com and www.culebraluxe.com and holds every production setting.
#
# Everything is compiled HERE, on this Mac, for free (deploy/Dockerfile.build: the Yew UI to WebAssembly, and the
# server cross-compiled for Vercel's x86_64 Linux). Vercel only unpacks the finished files (deploy/Dockerfile.runtime),
# so it spends seconds, not a paid 14-minute compile.
set -euo pipefail

TEAM_ID="team_xk8vFaeSyY6CuSkS3OK55tTc"
PROJECT_ID="prj_RHzXYauXOgIh2abiEsMiQJ1V3jlV"   # culebraluxe-web-fp
SITE="https://www.culebraluxe.com"
VERCEL_CLI_VERSION="59.25.4"

vc() { npx --yes "vercel@${VERCEL_CLI_VERSION}" "$@"; }
fail() { printf '\nERROR: %s\n' "$1" >&2; exit 1; }

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe repository"
cd "$ROOT_DIR"
vc whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"
docker info >/dev/null 2>&1 || fail "Docker is not running. Start Docker Desktop."
printf '\nCulebraLuxe production deploy\n  commit: %s\n' "$(git rev-parse --short HEAD)"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

printf '\n1/5 Building the stylesheet...\n'
npx --no-install tailwindcss -i web/ui/styles/app.css -o public/app.css --minify || fail "The stylesheet build failed."

printf '\n2/5 Compiling the application on this Mac (the first run is slow; later runs reuse the cache)...\n'
docker build -f deploy/Dockerfile.build --output "type=local,dest=$WORK/build" . || fail "The local compile failed."
[ -f "$WORK/build/culebraluxe" ] && [ -f "$WORK/build/ui_bg.wasm" ] || fail "The compile produced no server or no wasm."

printf '\n3/5 Packing the finished files...\n'
STAGE="$WORK/stage"
mkdir -p "$STAGE/public/rust-ui" "$STAGE/templates"
cp deploy/Dockerfile.runtime "$STAGE/Dockerfile"
# THE BUILD STAMP IS THE DEPLOYED COMMIT, written into the image that serves it. `/api/build-info` reads these two at
# runtime; the production smoke asserts the sha it reports is HEAD (or names the commits it is behind). Stamping here
# rather than at compile time is what makes the answer true: the binary is built once and the container is rebuilt per
# deploy, so the commit that travels with the image is the commit that is live. If this line is ever skipped the
# endpoint answers with no sha AND SAYS SO, and the release check fails loudly instead of comparing against nothing.
printf 'ENV CULEBRALUXE_BUILD_SHA=%s\nENV CULEBRALUXE_BUILT_AT=%s\n' \
  "$(git rev-parse HEAD)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$STAGE/Dockerfile"
gzip -9 -c "$WORK/build/culebraluxe" > "$STAGE/culebraluxe.gz"
rsync -a --exclude rust-ui --exclude '* 2.*' --exclude '* 2' public/ "$STAGE/public/"
cp "$WORK/build/ui.js" "$STAGE/public/rust-ui/ui.js"
gzip -9 -c "$WORK/build/ui_bg.wasm" > "$STAGE/public/rust-ui/ui_bg.wasm.gz"
rsync -a middle/model/forms/templates/ "$STAGE/templates/"
printf '  upload size: %s\n' "$(du -sh "$STAGE" | cut -f1)"

printf '\n4/5 Making sure the project runs the application container...\n'
printf '{"framework":"container"}' > "$WORK/project.json"
vc api "/v9/projects/${PROJECT_ID}?teamId=${TEAM_ID}" -X PATCH --input "$WORK/project.json" >/dev/null \
  || fail "Could not set the project to run a container."

printf '\n5/5 Deploying...\n'
(cd "$STAGE" && VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$PROJECT_ID" vc deploy --prod --yes) \
  || fail "The deploy failed. Production is unchanged."

printf '\nChecking %s...\n' "$SITE"
bad=0
for path in / /buyers /app.css /rust-ui/ui.js /rust-ui/ui_bg.wasm "/api/rust-ui/public-page?screen=site-home" /login; do
  code="$(curl -s -o /dev/null -w '%{http_code}' "${SITE}${path}")"
  printf '  %s  %s\n' "$code" "$path"
  [ "$code" = "200" ] || bad=1
done
[ "$bad" = 0 ] && printf '\nDEPLOY COMPLETE — %s serves the new application.\n' "$SITE" \
  || printf '\nDeployed, but a check above is not 200. To roll back: Vercel -> culebraluxe-web-fp -> Deployments -> promote the previous one.\n'
