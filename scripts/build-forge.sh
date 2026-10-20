#!/usr/bin/env bash
# Build a versioned Forge release artifact. No deploy, no DB, no VCS.
#
#   bash scripts/build-forge.sh [--skip-tests]
#
# Output: build/forge/<version>/ + build/forge-<version>.tar.gz
# The artifact carries the Forge binaries, the SDLC definition, the scheduler wrapper and a sha256
# manifest. "version" is the commit short SHA unless FORGE_BUILD_VERSION is set.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "ERROR: run inside the repository" >&2; exit 1; }
cd "$ROOT"
FORGE_ROOT="${FORGE_HOME:-$ROOT/../forge}"
[[ -f "$FORGE_ROOT/Cargo.toml" ]] || { echo "ERROR: Forge checkout not found at $FORGE_ROOT (set FORGE_HOME)" >&2; exit 1; }
SKIP_TESTS=0
[[ "${1:-}" == "--skip-tests" ]] && SKIP_TESTS=1

VERSION="${FORGE_BUILD_VERSION:-$(git -C "$FORGE_ROOT" rev-parse --short HEAD)}"
OUT="${FORGE_BUILD_DIR:-$ROOT/build/forge/$VERSION}"
STAGE="$OUT"
printf '\nForge release build\n  version: %s\n  output:  %s\n\n' "$VERSION" "$OUT"

if [[ "$SKIP_TESTS" == 0 ]]; then
  printf '1/4 Testing the factory crate...\n'
  cargo test -q --manifest-path "$FORGE_ROOT/Cargo.toml" -p forge --lib
else
  printf '1/4 Tests skipped (--skip-tests)\n'
fi

printf '2/4 Compiling release binaries...\n'
cargo build --release --manifest-path "$FORGE_ROOT/Cargo.toml" -p forge --bins

printf '3/4 Staging the artifact...\n'
# Lanes set a per-checkout target-dir in .cargo/config.toml; ask cargo where the release output is
# instead of assuming target/release (the lane's target is outside the checkout).
TARGET_DIR="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/definitions" "$STAGE/scripts"
for bin in forge forge-worker forge-task forge-assay-plan-hash; do
  src="$TARGET_DIR/release/$bin"
  [[ -x "$src" ]] || { echo "ERROR: missing release binary: $src" >&2; exit 1; }
  cp "$src" "$STAGE/bin/$bin"
done
cp "$FORGE_ROOT/forge/definitions/FORGE_SDLC-v6.xml" "$STAGE/definitions/"
cp "$FORGE_ROOT/agent-worker-once.sh" "$STAGE/scripts/"
chmod -R a+rX "$STAGE"
printf 'version=%s\ncommit=%s\nbuilt_at=%s\n' "$VERSION" "$(git rev-parse HEAD)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$STAGE/VERSION"
(cd "$STAGE" && find bin definitions scripts VERSION -type f -print0 | sort -z | xargs -0 shasum -a 256) > "$STAGE/MANIFEST.sha256"
(cd "$ROOT/build" && tar -czf "forge-$VERSION.tar.gz" "forge/$VERSION")
printf '  binaries: %s\n' "$(cd "$STAGE/bin" && ls | tr '\n' ' ')"
printf '  artifact: %s (%s)\n' "$ROOT/build/forge-$VERSION.tar.gz" "$(du -h "$ROOT/build/forge-$VERSION.tar.gz" | cut -f1)"

printf '4/4 Verifying the staged binaries...\n'
file "$STAGE/bin/forge-worker" | sed 's/^/  /'
"$STAGE/bin/forge" --version 2>/dev/null || true
printf '\nFORGE BUILD COMPLETE — %s\n' "$STAGE"
printf 'Deploy it with: bash scripts/deploy-forge-prod.sh --version %s\n' "$VERSION"
