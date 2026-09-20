#!/usr/bin/env bash
# Build the Rust UI to WebAssembly for the Next host.
#
# WHY THIS EXISTS INSTEAD OF `wasm-pack build`: wasm-pack passes `--out-dir` straight through to `cargo build`, which
# renamed that flag to `--artifact-dir`, so wasm-pack dies with "unexpected argument '--out-dir' found" before it
# compiles a single line. The two commands below are what wasm-pack does internally: cargo builds the cdylib, then the
# wasm-bindgen CLI generates the JavaScript glue. The CLI is pinned to the version this crate depends on (0.2.128),
# because a version mismatch between the crate and the CLI is a hard error by design.
#
# DELETE THIS SCRIPT and go back to `wasm-pack build` when wasm-pack understands the current cargo.
#
# Output split, and the split matters:
#   lib/rust-ui/ui.js   the importable ES module (Next bundles it; it is source, not an asset)
#   public/rust-ui/ui_bg.wasm   served as a static file, because the host passes its URL to `init` explicitly. The
#                               glue's default "next to this module" URL cannot work once a bundler owns the JS.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
js_out="$root/lib/rust-ui"
wasm_out="$root/public/rust-ui"
target_dir="${RUST_UI_TARGET_DIR:-$root/rust/target}"

if [ "${RUST_UI_PROFILE:-debug}" = "release" ]; then
  profile_dir="release"
else
  profile_dir="debug"
fi

# One array, never empty: bash 3.2 (the one macOS ships) treats an empty array under `set -u` as an unbound variable,
# which is a silly way to lose a build.
build_args=(
  --manifest-path "$root/rust/Cargo.toml"
  -p ui
  --features wasm
  --target wasm32-unknown-unknown
  --target-dir "$target_dir"
)
if [ "$profile_dir" = "release" ]; then
  build_args+=(--release)
fi

echo "==> cargo build (ui, wasm32-unknown-unknown, $profile_dir)"
cargo build "${build_args[@]}"

echo "==> wasm-bindgen"
mkdir -p "$js_out" "$wasm_out"
wasm-bindgen \
  --target web \
  --out-dir "$js_out" \
  --out-name ui \
  "$target_dir/wasm32-unknown-unknown/$profile_dir/ui.wasm"

# The wasm itself is fetched by URL, so it has to be reachable as a static asset.
cp "$js_out/ui_bg.wasm" "$wasm_out/ui_bg.wasm"

echo "==> done"
echo "    JS glue:  lib/rust-ui/ui.js"
echo "    WASM:     public/rust-ui/ui_bg.wasm ($(du -h "$wasm_out/ui_bg.wasm" | cut -f1))"
