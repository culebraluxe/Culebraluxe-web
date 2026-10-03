#!/usr/bin/env bash
# Everything, locally: the Rust server, and the Rust UI's WASM + stylesheet.
#
# The deploy does NOT run this, and should not: the whole application is compiled on this Mac by
# `scripts/deploy-prod.sh` inside `devops/Dockerfile.build`, rather than on whatever machine you are holding. This
# script is for verifying a complete build before you push, and for producing the WASM artifact the site ships.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo '==> rust workspace check'
cargo check --workspace --all-targets

echo '==> rust server (release, exactly what the container builds)'
cargo build --release -p web --bin web

echo '==> rust tests'
cargo test -p db -p web -p forge -p workflow

echo '==> rust ui (wasm) + stylesheet — the same two steps `pnpm build` runs'
RUST_UI_PROFILE=release bash scripts/site-build.sh

echo
echo 'built:'
echo "  server  ${CARGO_TARGET_DIR:-target}/release/web"
echo "  wasm    public/rust-ui/ui_bg.wasm + ui.js"
echo "  css     public/app.css"
