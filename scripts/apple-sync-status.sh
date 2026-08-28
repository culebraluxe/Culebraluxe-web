#!/usr/bin/env bash
# CulebraLuxe — Apple sync status (installed/loaded/PID/last exit/last success/log).
set -euo pipefail
cd "$(dirname "$0")"
exec node apple-sync-agent.mjs status "$@"
