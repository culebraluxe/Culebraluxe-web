#!/usr/bin/env bash
# Dogfood entrypoint for the first Harness Evolution experiment.
#
# READ-ONLY BY DEFAULT: prints the typed dry-run plan (base revision, targets, candidates, probe
# commands, expected evidence) and the requirements for a real run. It calls no model, reads/writes
# no database and never moves the active harness revision.
#
#   scripts/harness-dogfood.sh                     dry-run plan
#   scripts/harness-dogfood.sh --authorize-sandbox  print the real-run boundary plan
#
# --authorize-sandbox is the explicit authorization precondition for a real run: it prints the
# authorized plan but still refuses to execute probes, because the probe entrypoint (O2), a real
# sandbox runtime and DATABASE_URL_DEV are not wired here.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

filter="engine::harness_evolution::dogfood::tests::print_dry_run_plan"
if [[ "${1:-}" == "--authorize-sandbox" ]]; then
  filter="engine::harness_evolution::dogfood::tests::print_authorized_real_run_boundary"
elif [[ $# -gt 0 ]]; then
  echo "usage: $0 [--authorize-sandbox]" >&2
  exit 2
fi
exec cargo test -q -p forge --lib "$filter" -- --nocapture
