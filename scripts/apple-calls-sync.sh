#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="${CULEBRALUXE_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}"
cd "$REPO_ROOT"

OUT_DIR="$REPO_ROOT/public/upload/data/apple-messages-export"
CALLS_FILE="$OUT_DIR/calls.jsonl"

[ -f .env.local ] || { echo "missing .env.local" >&2; exit 1; }
command -v swift >/dev/null 2>&1 || { echo "swift not found" >&2; exit 1; }
command -v node >/dev/null 2>&1 || { echo "node not found" >&2; exit 1; }

mkdir -p "$OUT_DIR"

echo "[apple-calls] read-only export start"
swift scripts/macbridge/AppleCallHistory.swift --out "$CALLS_FILE"

echo "[apple-calls] PROD intake start"
node --env-file=.env.local --import tsx scripts/apple-calls-intake.ts prod --file "$CALLS_FILE"

echo "[apple-calls] complete"
