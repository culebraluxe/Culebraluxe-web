#!/bin/bash
# The repo root is derived from this script's own location, so it does not name a path that the
# 2026-10-01 move invalidated (it said ~/Documents/Culebraluxe-web, which no longer holds the repo).
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)" || exit 1
story="$1"
# Persistent log dir: /tmp is periodically swept on macOS and took the run logs with
# it (the DB is the source of truth, but the chain log is what makes a run debuggable).
# It is the shared machine log tree, outside every checkout (docs/agent/LAYOUT.md).
logdir="${CULEBRALUXE_FORGE_LOG_DIR:-/Users/Shared/dev/build/logs/forge}"
mkdir -p "$logdir"
nohup pnpm forge:engine -- --story "$story" --work-type FEATURE > "$logdir/$story.log" 2>&1 &
echo "pid=$! story=$story log=$logdir/$story.log"
