#!/usr/bin/env bash
# Everything, locally: the Rust server, the Rust UI's WASM, and the Next frontend.
#
# The deploy does NOT run this, and should not: Vercel builds each service separately, and the server is built inside a
# Linux container (`rust/Dockerfile.vercel`) rather than on whatever machine you are holding. This script is for
# verifying a complete build before you push, and for producing the WASM artifact that the frontend ships.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo '==> rust workspace check'
(cd rust && cargo check --workspace --all-targets)

echo '==> rust server (release, exactly what the container builds)'
(cd rust && cargo build --release -p server --bin http)

echo '==> rust tests'
(cd rust && cargo test -p db -p server -p forge -p workflow)

echo '==> rust ui (wasm) + next'
node scripts/build.mjs

echo
echo 'built:'
echo "  server  rust/target/release/http"
echo "  wasm    lib/rust-ui/ui_bg.wasm + ui.js"
echo "  next    .next/"
