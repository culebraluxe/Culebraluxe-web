#!/usr/bin/env bash
# CulebraLuxe — uninstall the Apple sync LaunchAgent + deployed copies.
set -euo pipefail
cd "$(dirname "$0")"
exec node apple-sync-agent.mjs uninstall "$@"
