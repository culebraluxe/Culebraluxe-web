#!/usr/bin/env bash
# Deploy a Forge release to the Mac launchd worker (PROD Neon). Not Vercel.
#
#   bash scripts/deploy-forge-prod.sh [--version <v>] [--dry-run] [--no-build]
#
# Steps: build/verify artifact -> install under ~/Library/Application Support/CulebraLuxe/forge/<v>
#        -> point `current` at it -> set AGENT_WORKER_BIN in .env.scheduler -> refresh the launchd
#        wrapper/plist -> report status. It NEVER runs a work pass (no story is claimed).
# Rollback: `bash scripts/deploy-forge-prod.sh --rollback` points `current` and the scheduler at the
#           previous release.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "ERROR: run inside the repository" >&2; exit 1; }
cd "$ROOT"
SUPPORT="${FORGE_SUPPORT_DIR:-$HOME/Library/Application Support/CulebraLuxe}"
VERSION=""; DRY_RUN=0; NO_BUILD=0; ROLLBACK=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --no-build) NO_BUILD=1; shift ;;
    --rollback) ROLLBACK=1; shift ;;
    *) echo "ERROR: unknown argument: $1" >&2; exit 1 ;;
  esac
done

fail() { printf '\nERROR: %s\n' "$1" >&2; exit 1; }

if [[ "$ROLLBACK" == 1 ]]; then
  PREV="$SUPPORT/forge/previous"
  [[ -d "$PREV" ]] || fail "no previous release at $PREV"
  NEW="$(readlink "$PREV")"
  ln -sfn "$NEW" "$SUPPORT/forge/current"
  [[ "$DRY_RUN" == 1 ]] || sed -i '' '/^# forge-deploy managed$/,/^AGENT_WORKER_BIN=/{/^AGENT_WORKER_BIN=/d;}' "$ROOT/.env.scheduler" 2>/dev/null || true
  [[ "$DRY_RUN" == 1 ]] || printf '# forge-deploy managed\nAGENT_WORKER_BIN=%s/forge/current/bin/forge-worker\n' "$SUPPORT" >> "$ROOT/.env.scheduler"
  printf 'Rolled back to %s\n' "$NEW"
  exit 0
fi

[[ -z "$(git status --porcelain)" ]] || fail "Working tree is not clean. Commit or discard changes first."
[[ "$NO_BUILD" == 1 ]] || bash "$ROOT/scripts/build-forge.sh"
VERSION="${VERSION:-${FORGE_BUILD_VERSION:-$(git rev-parse --short HEAD)}}"
SRC="$ROOT/build/forge/$VERSION"
[[ -d "$SRC" ]] || fail "artifact not found: $SRC (run scripts/build-forge.sh)"
DEST="$SUPPORT/forge/$VERSION"

printf '\nForge production deploy\n  version: %s\n  target:  %s\n  commit:  %s\n\n' "$VERSION" "$DEST" "$(git rev-parse --short HEAD)"
printf '1/5 Installing the artifact...\n'
if [[ "$DRY_RUN" == 1 ]]; then
  printf '  DRY-RUN: would rsync %s -> %s\n' "$SRC" "$DEST"
else
  mkdir -p "$SUPPORT/forge"
  rm -rf "$DEST"
  cp -R "$SRC" "$DEST"
  chmod -R a+rX "$DEST"
fi

printf '2/5 Pointing current/previous at the release...\n'
if [[ "$DRY_RUN" == 1 ]]; then
  printf '  DRY-RUN: would symlink current -> %s\n' "$DEST"
else
  if [[ -L "$SUPPORT/forge/current" ]]; then
    ln -sfn "$(readlink "$SUPPORT/forge/current")" "$SUPPORT/forge/previous"
  fi
  ln -sfn "$DEST" "$SUPPORT/forge/current"
fi

printf '3/5 Wiring the scheduler to the release binary...\n'
ENVFILE="$ROOT/.env.scheduler"
if [[ "$DRY_RUN" == 1 ]]; then
  printf '  DRY-RUN: would set AGENT_WORKER_BIN=%s/forge/current/bin/forge-worker in %s\n' "$SUPPORT" "$ENVFILE"
else
  touch "$ENVFILE"
  chmod 600 "$ENVFILE"
  # remove a previous forge-deploy block, keep everything else
  awk 'BEGIN{skip=0} /^# forge-deploy managed$/{skip=1;next} skip && /^AGENT_WORKER_BIN=/{skip=0;next} {print}' "$ENVFILE" > "$ENVFILE.tmp"
  mv "$ENVFILE.tmp" "$ENVFILE"
  printf '# forge-deploy managed\nAGENT_WORKER_BIN=%s/forge/current/bin/forge-worker\n' "$SUPPORT" >> "$ENVFILE"
fi

printf '4/5 Refreshing the launchd wrapper/plist...\n'
if [[ "$DRY_RUN" == 1 ]]; then
  printf '  DRY-RUN: would run cargo run -p forge-cli -- launchd agent-worker install\n'
else
  cargo run -q -p forge-cli -- launchd agent-worker install
fi

printf '5/5 Status...\n'
if [[ "$DRY_RUN" == 1 ]]; then
  printf '  DRY-RUN: would run launchd agent-worker status\n'
else
  cargo run -q -p forge-cli -- launchd agent-worker status || true
fi

printf '\nFORGE DEPLOY COMPLETE — %s\n' "$DEST"
printf 'The scheduler will use %s/bin/forge-worker on its next 3-minute wake.\n' "$SUPPORT/forge/current"
printf 'No work pass was run. To run one manually: pnpm agent:scheduler:run\n'
printf 'Rollback: bash scripts/deploy-forge-prod.sh --rollback\n'
