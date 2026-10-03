#!/usr/bin/env bash
# Build the application image and prove it runs — a DRY RUN: nothing is pushed or deployed.
#
#   bash scripts/site-container.sh          build the release site (wasm + CSS), build the image, run it against DEV
#                                           on http://localhost:8090, check the pages and assets answer, stop it.
#   SKIP_SITE_BUILD=1 ...                   reuse the already-built public/ (faster when only Rust changed)
#
# The container reads DEV settings from .env.local. It runs with APP_ENV=development, so the portal's ROOT stub
# works for the check; a production deploy sets APP_ENV=production, which refuses the stub.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
image="culebraluxe-site:dry-run"
port="${SITE_CONTAINER_PORT:-8090}"

# The wasm is built inside the image; only the stylesheet is built here.
if [ -z "${SKIP_SITE_BUILD:-}" ]; then
  npx --no-install tailwindcss -i web/ui/styles/app.css -o public/app.css --minify
fi

echo "==> docker build ($image)"
docker build -t "$image" .

env_file="$(mktemp)"
trap 'rm -f "$env_file"; docker rm -f culebraluxe-dry-run >/dev/null 2>&1 || true' EXIT
for key in DATABASE_URL_DEV AUTH_SECRET CULEBRA_INTERNAL_API_KEY; do
  value="$(grep -m1 "^${key}=" .env.local 2>/dev/null | cut -d= -f2- | sed -e 's/^"//' -e 's/"$//' || true)"
  [ -n "$value" ] && printf '%s=%s\n' "$key" "$value" >> "$env_file"
done
printf 'APP_ENV=development\nCULEBRA_UI_AUTH_STUB=root\n' >> "$env_file"

echo "==> docker run on http://localhost:${port}"
docker rm -f culebraluxe-dry-run >/dev/null 2>&1 || true
docker run -d --name culebraluxe-dry-run --env-file "$env_file" -p "${port}:8080" "$image" >/dev/null

for _ in $(seq 1 90); do
  curl -fsS "http://localhost:${port}/healthz" >/dev/null 2>&1 && break
  sleep 2
done

failed=0
for path in /healthz / /buyers /portal/dashboard /app.css /rust-ui/ui.js /rust-ui/ui_bg.wasm /api/portal/rust-ui/entitlements \
            "/api/rust-ui/public-page?screen=site-home"; do
  code="$(curl -s -o /dev/null -w '%{http_code}' "http://localhost:${port}${path}")"
  printf '  %s  %s\n' "$code" "$path"
  [ "$code" = "200" ] || failed=1
done
if [ "$failed" = 0 ]; then
  echo "==> dry run passed: the image serves the site, the portal and the API"
else
  echo "==> dry run FAILED — container log:"; docker logs --tail 40 culebraluxe-dry-run
  exit 1
fi
