#!/usr/bin/env bash
# Dogfood entrypoint for the first Harness Evolution experiment.
#
# READ-ONLY BY DEFAULT: prints the typed dry-run plan (base revision, targets, candidates, probe
# commands, expected evidence) and the requirements for a real run. It calls no model, reads/writes
# no database and never moves the active harness revision.
#
#   scripts/harness-dogfood.sh                       dry-run plan
#   scripts/harness-dogfood.sh --authorize-sandbox    real-run boundary plan (H5 sandbox token)
#   scripts/harness-dogfood.sh --authorize-model-run  D1 model-run boundary (paid-turn token)
#
# The two authorization flags are separate gates: --authorize-sandbox proves a real run may touch a
# sandbox; --authorize-model-run opens the paid model turn. Neither executes a probe: the probe
# entrypoint (O2), the model-turn executor wiring and DATABASE_URL_DEV are not wired in this build,
# so the authorized modes print the boundary and refuse to pretend a run happened. No live paid
# model call is made by this script.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

case "${1:-}" in
  --authorize-sandbox)
    filter="engine::harness_evolution::dogfood::tests::print_authorized_real_run_boundary"
    ;;
  --authorize-model-run)
    filter="engine::harness_evolution::dogfood::tests::print_authorized_model_run_boundary"
    ;;
  "")
    filter="engine::harness_evolution::dogfood::tests::print_dry_run_plan"
    ;;
  *)
    echo "usage: $0 [--authorize-sandbox | --authorize-model-run]" >&2
    exit 2
    ;;
esac
exec cargo test -q -p forge --lib "$filter" -- --nocapture
