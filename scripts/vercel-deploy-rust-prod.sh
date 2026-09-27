#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# Deploy ONLY the Rust API container (project: culebraluxe-rust-api), from
# rust/Dockerfile.vercel.
#
# WHY THIS EXISTS. The release script deploys the frontend; the cutover script
# deploys the container but also clones production env into projects and is far
# too much machinery to run for a server change. So server changes had no
# ordinary path, and the chunked-upload endpoints shipped to a container that
# was still serving the previous code — the frontend called routes that did not
# exist and the screen said "upload failed" with no clue why.
#
# This does one thing, and then PROVES it: the container is asked whether it
# knows the routes it is supposed to have. A deploy that reports success while
# the routes are missing is the exact failure this guards against.
# ---------------------------------------------------------------------------

TEAM_ID="team_xk8vFaeSyY6CuSkS3OK55tTc"
RUST_PROJECT_NAME="culebraluxe-rust-api"
VERCEL_CLI_VERSION="59.25.4"

vc() {
  npx --yes "vercel@${VERCEL_CLI_VERSION}" "$@"
}

fail() {
  printf '\nERROR: %s\n' "$1" >&2
  exit 1
}

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "Run this inside the CulebraLuxe git repository"
cd "$ROOT_DIR"

# A NOTE, NOT A BLOCKER. The container is built from the working tree on Vercel's side, so uncommitted edits would ship
# in it — worth saying out loud, not worth stopping the release the frontend has already gone out with.
if ! git diff --quiet --ignore-submodules -- . ':(exclude)docs/agent/manifest'; then
  printf '\nNOTE: the container will be built from a tree with local changes.\n'
fi

vc whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"

printf '\nCulebraLuxe Rust API deploy\n'
printf '  commit:  %s\n' "$(git rev-parse --short HEAD)"

printf '\nResolving the container project...\n'
RUST_PROJECT_ID="$(
  vc api "/v9/projects/${RUST_PROJECT_NAME}?teamId=${TEAM_ID}" 2>/dev/null |
    node -e 'let raw="";process.stdin.on("data",(d)=>raw+=d).on("end",()=>{const p=(JSON.parse(raw).project)??JSON.parse(raw);if(!p.id)process.exit(1);process.stdout.write(p.id)})'
)" || fail "Could not resolve the Rust project '${RUST_PROJECT_NAME}'."
printf '  project: %s (%s)\n' "$RUST_PROJECT_NAME" "$RUST_PROJECT_ID"

printf '\nBuilding the stylesheet (the wasm is built inside the image)...\n'
(cd "$ROOT_DIR" && npx --no-install tailwindcss -i rust/ui/styles/app.css -o public/app.css --minify) \
  || fail "The stylesheet build failed. Nothing was shipped."

# THE WHOLE APPLICATION IS ONE IMAGE: the Rust server, the built site (public/) and the form templates. The CLI
# deploys one folder, so it is assembled here — rust/ without its build output, public/, lib/forms/templates — with
# the repository's Dockerfile (proven by `pnpm container:dry-run`) under both names the project may build from.
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
printf '\nAssembling the image context in %s...\n' "$STAGE"
rsync -a --exclude target --exclude '* 2.*' --exclude '* 2' "$ROOT_DIR/rust/" "$STAGE/rust/"
rsync -a --exclude '* 2.*' --exclude rust-ui "$ROOT_DIR/public/" "$STAGE/public/"
mkdir -p "$STAGE/scripts" && cp "$ROOT_DIR/scripts/rust-ui-build.sh" "$STAGE/scripts/rust-ui-build.sh"
mkdir -p "$STAGE/lib/forms" && rsync -a "$ROOT_DIR/lib/forms/templates/" "$STAGE/lib/forms/templates/"
cp "$ROOT_DIR/Dockerfile" "$STAGE/Dockerfile"
cp "$ROOT_DIR/Dockerfile" "$STAGE/Dockerfile.vercel"
[ -f "$STAGE/public/app.css" ] || fail "The stylesheet is missing from the image context."

printf '\nDeploying the application image...\n'
cd "$STAGE"
if ! VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$RUST_PROJECT_ID" vc deploy --prod --yes; then
  fail "The container deploy failed. Nothing was shipped."
fi
cd "$ROOT_DIR"

printf '\nVerifying the site answers from the container...\n'
for path in / /buyers /app.css /rust-ui/ui.js /rust-ui/ui_bg.wasm "/api/rust-ui/public-page?screen=site-home"; do
  code="$(curl -s -o /dev/null -w '%{http_code}' "https://culebraluxe-rust-api.vercel.app${path}")"
  printf '  %s  %s\n' "$code" "$path"
done

printf '\nVerifying the live container knows its routes...\n'
node --input-type=module -e '
const base = "https://culebraluxe-rust-api.vercel.app"
const expected = [
  ["/v1/properties/00000000-0000-0000-0000-000000000000/media", "the media upload route"],
  ["/v1/properties/00000000-0000-0000-0000-000000000000/media/uploads", "the chunked upload route"],
]
let bad = 0
for (const [path, label] of expected) {
  try {
    const response = await fetch(base + path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: "{}",
    })
    const ok = response.status !== 404
    if (!ok) bad += 1
    console.log(`  ${ok ? "ok  " : "MISSING"} ${response.status}  ${label}`)
  } catch (error) {
    bad += 1
    console.log(`  FAIL ${label}: ${error.message}`)
  }
}
process.exit(bad === 0 ? 0 : 1)
' || fail "The container deployed but does not serve the expected routes. It is running the previous code."

printf '\nRUST API DEPLOY COMPLETE — routes verified on the live container.\n'
