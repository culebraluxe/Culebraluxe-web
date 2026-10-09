#!/usr/bin/env bash
# PRODUCTION DEPLOY — the CulebraLuxe application to its one Vercel project, culebraluxe-web-fp, which serves
# culebraluxe.com and www.culebraluxe.com and holds every production setting.
#
# Everything is compiled HERE, on this Mac, for free (devops/Dockerfile.build: the Yew UI to WebAssembly, and the
# server cross-compiled for Vercel's x86_64 Linux). Vercel only unpacks the finished files (devops/Dockerfile.runtime),
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

# RELEASE GUARDS — main branch, clean tree, or no deploy. The posture is copied from the superseded
# `scripts/vercel-deploy-prod.sh:32-50`, hardened from a warning into a refusal: that script uploaded a
# PREBUILT artifact, so uncommitted source could not reach production through it and a note sufficed. THIS
# script compiles the working tree (`docker build .`, step 2/5 below), so a dirty tree would ship — the
# guard refuses before anything builds instead of noting it afterwards.
#
# `--dry-run` runs the guards and stops before touching Docker or Vercel: the assay for this guard is a
# dry run from a dirty fixture checkout, which must refuse.
DRY_RUN=0
if [ "${1:-}" = "--dry-run" ] || [ "${1:-}" = "--check-only" ]; then DRY_RUN=1; fi

BRANCH="$(git branch --show-current)"
[ "$BRANCH" = "main" ] || fail "Production deploys must run from main. Current branch: ${BRANCH:-detached}"
# GENERATED MANIFESTS ARE EXEMPT. `docs/agent/manifest/*.md` carries a render timestamp and a "last touched"
# date per row, so a release build rewrites them by definition — failing the deploy over that would mean
# committing generated files between the build and the deploy.
DEPLOY_EXEMPT=':(exclude)docs/agent/manifest'
if ! git diff --quiet --ignore-submodules -- . "$DEPLOY_EXEMPT"; then
  git status --short | head -20 >&2
  fail "Tracked files have local changes (see above) — commit or stash them before deploying production."
fi
if ! git diff --cached --quiet --ignore-submodules -- . "$DEPLOY_EXEMPT"; then
  fail "Staged files are waiting to be committed — commit them before deploying production."
fi
DEPLOY_SHA="$(git rev-parse HEAD)"
if [ "$DRY_RUN" = 1 ]; then
  printf 'DRY RUN — guards pass on %s at %s; no build, no deploy.\n' "$BRANCH" "$(git rev-parse --short HEAD)"
  exit 0
fi
vc whoami >/dev/null 2>&1 || fail "Vercel CLI is not authenticated. Run: vercel login"
docker info >/dev/null 2>&1 || fail "Docker is not running. Start Docker Desktop."
printf '\nCulebraLuxe production deploy\n  commit: %s\n' "$(git rev-parse --short HEAD)"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

printf '\n1/5 Building the stylesheet...\n'
npx --no-install tailwindcss -i web/ui/styles/app.css -o public/app.css --minify || fail "The stylesheet build failed."

printf '\n2/5 Compiling the application on this Mac (the first run is slow; later runs reuse the cache)...\n'
docker build -f devops/Dockerfile.build --output "type=local,dest=$WORK/build" . || fail "The local compile failed."
[ -f "$WORK/build/culebraluxe" ] && [ -f "$WORK/build/ui_bg.wasm" ] || fail "The compile produced no server or no wasm."

printf '\n3/5 Packing the finished files...\n'
STAGE="$WORK/stage"
mkdir -p "$STAGE/public/rust-ui" "$STAGE/templates"
cp devops/Dockerfile.runtime "$STAGE/Dockerfile"
# THE BUILD STAMP IS THE DEPLOYED COMMIT, written into the image that serves it. `/api/build-info` reads these two at
# runtime; the production smoke asserts the sha it reports is HEAD (or names the commits it is behind). Stamping here
# rather than at compile time is what makes the answer true: the binary is built once and the container is rebuilt per
# deploy, so the commit that travels with the image is the commit that is live. If this line is ever skipped the
# endpoint answers with no sha AND SAYS SO, and the release check fails loudly instead of comparing against nothing.
printf 'ENV CULEBRALUXE_BUILD_SHA=%s\nENV CULEBRALUXE_BUILT_AT=%s\n' \
  "$(git rev-parse HEAD)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$STAGE/Dockerfile"
gzip -9 -c "$WORK/build/culebraluxe" > "$STAGE/culebraluxe.gz"
# PRIVATE LOCAL DATA MUST NOT SHIP. `public/` is the webroot AND an rsync source, and rsync does not read
# `.gitignore` — so `public/upload/data/`, which `.gitignore:42` calls private ("private export data (never
# commit)"), was copied into the image byte for byte. It stayed harmless only for as long as mode-600 files
# were unreadable to the container's non-root user; the day the stage started normalizing modes the whole Apple
# Messages export became a public 64 MB download (2026-10-05, found by the post-deploy sweep). The sibling
# artifact path already refuses exactly this and fails its build when the export is traced in
# (scripts/vercel-build-prod.sh:143-154, "Private local data was traced into the prebuilt artifact"); the
# container path lost that check in the layout refactor. It gets both halves back here: the exclusion below, and
# the fail-closed probe after the stage is filled. Excluding `upload/data` closes the class, not the one file —
# it is the staging root the exporters write to, and the only other thing in it is `macdatabridge.swift`, a
# helper source, plus empty placeholders, so nothing the site serves is lost.
rsync -a --exclude rust-ui --exclude 'upload/data' --exclude '* 2.*' --exclude '* 2' public/ "$STAGE/public/"
cp "$WORK/build/ui.js" "$STAGE/public/rust-ui/ui.js"
gzip -9 -c "$WORK/build/ui_bg.wasm" > "$STAGE/public/rust-ui/ui_bg.wasm.gz"
rsync -a middle/model/forms/templates/ "$STAGE/templates/"
# THE IMAGE RUNS AS A NON-ROOT USER — devops/Dockerfile.runtime adds `culebra` as uid 10001 and ends with
# `USER culebra` — and a Docker `COPY` hands the container exactly the modes it is given. So a 600 file or a 700
# directory in the working tree ships as an asset the server cannot open: every image under /images is unreadable and
# every form template fails to load, while this script's own checks (all of which are HTML, wasm or API paths) stay
# green and report a clean deploy. The working tree is not the right place to fix it — a developer's umask, or a copy
# made by iCloud, is not a release decision, and `rsync -a` deliberately preserves what it finds — so the STAGE is
# normalized instead, after everything has been copied into it and before any of it is packed.
chmod -R a+rX "$STAGE/public" "$STAGE/templates"
# FAIL CLOSED, never warn: an artifact carrying private local data is not deployed — the same posture the sibling
# path takes. The paths are the sibling's list, widened to the whole staging root so a future export cannot slip
# through under a new filename.
PRIVATE_MATCH="$(find "$STAGE" \( -path '*/upload/data/*' -o -path '*/apple-messages-export/*' \
  -o -path '*/contact-export/*' -o -path '*/apple-messages-output/*' -o -name 'culebraluxe-calendar*.json' \) \
  -print -quit 2>/dev/null || true)"
[ -z "$PRIVATE_MATCH" ] || fail "Private local data reached the deploy stage: $PRIVATE_MATCH — do not deploy this artifact."
printf '  private-data probe: clean\n'
printf '  upload size: %s\n' "$(du -sh "$STAGE" | cut -f1)"

printf '\n4/5 Making sure the project runs the application container...\n'
printf '{"framework":"container"}' > "$WORK/project.json"
vc api "/v9/projects/${PROJECT_ID}?teamId=${TEAM_ID}" -X PATCH --input "$WORK/project.json" >/dev/null \
  || fail "Could not set the project to run a container."

printf '\n5/5 Deploying...\n'
(cd "$STAGE" && VERCEL_ORG_ID="$TEAM_ID" VERCEL_PROJECT_ID="$PROJECT_ID" vc deploy --prod --yes) \
  || fail "The deploy failed. Production is unchanged."

printf '\nChecking %s...\n' "$SITE"
ROLLBACK="Vercel -> culebraluxe-web-fp -> Deployments -> promote the previous one."
bad=0
for path in / /buyers /app.css /rust-ui/ui.js /rust-ui/ui_bg.wasm "/api/rust-ui/public-page?screen=site-home" /login; do
  code="$(curl -s -o /dev/null -w '%{http_code}' "${SITE}${path}")"
  printf '  %s  %s\n' "$code" "$path"
  [ "$code" = "200" ] || bad=1
done
[ "$bad" = "0" ] || fail "Deployed, but a check above is not 200. To roll back: ${ROLLBACK}."

# LIVE SMOKE — ask production whether it WORKS, not just whether it answers 200. `smoke prod --expect-head`
# asserts the media contract (the hero nested inside the Rust PropertyRecord with a non-empty gallery,
# `cli/src/smoke.rs`) and that the container resolved `databaseTarget==prod`, plus that the live sha equals
# the commit just deployed. A mismatch fails the release: production is answering but not behaving.
# This is the same command `pnpm smoke:prod` runs, with `--expect-head` folding the sha assertion in.
printf '\nLive smoke (does production actually work)...\n'
if ! cargo run -q --manifest-path "$ROOT_DIR/Cargo.toml" -p cli -- smoke prod --expect-head; then
  fail "Deployed and answering, but the live smoke failed — see the checks above. To roll back: ${ROLLBACK}."
fi

# DEPLOY RECEIPT — the sha plus the migration ledger state, so code-vs-migration skew is visible after the
# fact. The deploy applies NO schema (docs/rust-prod-checklist.md: a migration is a separate explicit action
# in the same release window as the code that needs it); this snapshot records what was applied where at
# deploy time. Best-effort by design: a database the operator cannot reach from here must not fail a deploy
# that already smoked green.
RECEIPT_DIR="$ROOT_DIR/docs/agent/deploys"
mkdir -p "$RECEIPT_DIR"
RECEIPT_FILE="$RECEIPT_DIR/$(date -u +%Y%m%dT%H%M%SZ)-$(git rev-parse --short HEAD).md"
LEDGER_STATE="$(cargo run -q --manifest-path "$ROOT_DIR/Cargo.toml" -p cli -- db-tool status 2>&1 \
  || printf 'ledger unreadable from the deploy host\n')"
{
  printf '# Deploy receipt — %s\n\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'sha: %s\nbranch: %s\nsite: %s\nsmoke: `smoke prod --expect-head` passed\n\n' \
    "$DEPLOY_SHA" "$BRANCH" "$SITE"
  printf '## Migration ledger at deploy time (`cli db-tool status`)\n\n```\n%s\n```\n' "$LEDGER_STATE"
} >"$RECEIPT_FILE"
git add "$RECEIPT_FILE" 2>/dev/null || true
if ! git diff --cached --quiet 2>/dev/null; then
  git commit -q -m "deploy: receipt ${DEPLOY_SHA:0:12}" -- "$RECEIPT_FILE" || true
  printf 'receipt committed: %s\n' "$RECEIPT_FILE"
else
  printf 'receipt: %s\n' "$RECEIPT_FILE"
fi

printf '\nDEPLOY COMPLETE AND SMOKED — %s serves the new application.\n' "$SITE"
