#!/usr/bin/env bash
# Immediate APP -> Apple pass. This deliberately does NOT run the Calendar or
# Reminders snapshot/intake cycle; the scheduled calendar sync owns inbound
# reconciliation. The trusted apple-sync-launcher executes this wrapper so Swift
# keeps the same proven macOS/TCC boundary as the scheduled job.
set -euo pipefail

export PATH="/opt/homebrew/opt/node@24/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:${PATH:-}"

if [ -n "${CULEBRALUXE_REPO:-}" ]; then
  REPO_ROOT="$CULEBRALUXE_REPO"
else
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd -P)"
fi

cd "$REPO_ROOT"

if [ ! -f "$REPO_ROOT/.env.local" ]; then
  echo "apple outbound: missing $REPO_ROOT/.env.local" >&2
  exit 1
fi

NODE_BIN="$(command -v node || true)"
if [ -z "$NODE_BIN" ]; then
  echo "apple outbound: node not found" >&2
  exit 1
fi

exec env APP_ENV=production EXECUTION_ENV=PROD \
  "$NODE_BIN" --env-file="$REPO_ROOT/.env.local" --import tsx \
  scripts/apple-gateway-worker.ts
