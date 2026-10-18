#!/usr/bin/env bash
# The dynamic orchestrator's read surface: one decision packet from the live Forge control plane.
#
#   scripts/orchestrate-status.sh                 # human summary
#   scripts/orchestrate-status.sh --format json   # the packet as JSON
#
# Read-only by construction: it calls the existing `forge sql` verb, which refuses non-SELECT
# statements and runs inside `begin read only`. It creates no queue and writes no state; dispatch and
# decision recording stay on the canonical Forge paths.
set -uo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT" || exit 2
exec python3 "$ROOT/scripts/orchestrate-status.py" "$@"
