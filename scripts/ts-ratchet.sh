#!/usr/bin/env bash
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
allowlist="$root/docs/agent/ts-allowlist.txt"
actual="$(mktemp)"
expected="$(mktemp)"
cleanup() { rm -f "$actual" "$expected"; }
trap cleanup EXIT

if [[ ! -f "$allowlist" ]]; then
  echo "missing docs/agent/ts-allowlist.txt" >&2
  exit 1
fi

while IFS= read -r -d '' file; do
  case "$file" in
    legacy/*) continue ;;
    *.ts|*.tsx|*.js|*.mjs|*.cjs) printf '%s\n' "$file" ;;
  esac
done < <(git -C "$root" ls-files -z) | LC_ALL=C sort -u > "$actual"

LC_ALL=C sort -u "$allowlist" > "$expected"

status=0
while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  echo "$file: new TS/JS is not allowed; write it in Rust" >&2
  status=1
done < <(comm -23 "$actual" "$expected")

while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  echo "remove $file from ts-allowlist.txt" >&2
  status=1
done < <(comm -13 "$actual" "$expected")

if [[ "$status" -ne 0 ]]; then
  exit "$status"
fi

count="$(wc -l < "$actual" | tr -d '[:space:]')"
echo "ts-ratchet — PASS: ${count} tracked TS/JS file(s) outside legacy/; allowlist matches"
