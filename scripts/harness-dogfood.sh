#!/usr/bin/env bash
# Dry-run the first Harness Evolution dogfood experiment plan.
#
# READ-ONLY: prints the typed plan (base revision, targets, candidates, probe commands, expected
# evidence) and the requirements for a real run. It calls no model, reads/writes no database and
# never moves the active harness revision.
#
# Usage: scripts/harness-dogfood.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
exec cargo test -q -p forge --lib \
  engine::harness_evolution::dogfood::tests::print_dry_run_plan -- --nocapture
