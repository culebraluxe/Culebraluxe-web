#!/usr/bin/env bash
# CulebraLuxe — install/refresh the Apple sync LaunchAgent (twice-daily).
set -euo pipefail
cd "$(dirname "$0")"
exec node apple-sync-agent.mjs install "$@"
