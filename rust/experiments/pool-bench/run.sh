#!/usr/bin/env bash
# Runs the pool comparison. The connection string comes from .env.local and is never printed.
#
#   bash rust/experiments/pool-bench/run.sh
#
# It builds this crate with its own target directory, so the workspace's target/ and lockfile are untouched.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
ENV_FILE="$ROOT/.env.local"

DATABASE_URL_DEV="$(grep -m1 '^DATABASE_URL_DEV=' "$ENV_FILE" | cut -d= -f2- | tr -d '"')"
if [ -z "$DATABASE_URL_DEV" ]; then echo "DATABASE_URL_DEV not found in $ENV_FILE"; exit 1; fi
export DATABASE_URL_DEV

cd "$HERE"
CARGO_TARGET_DIR="$HERE/target" exec cargo run --quiet --release
