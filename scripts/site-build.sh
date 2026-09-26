#!/usr/bin/env bash
# Build everything the Rust server serves as the website (rust/server/src/site.rs): the Yew wasm and its glue, then the
# stylesheet. Tailwind scans the Rust sources for class names, so the CSS is built after the UI source is final.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$root/scripts/rust-ui-build.sh"
echo "==> tailwind (rust/ui/styles/app.css -> public/app.css)"
(cd "$root" && npx --no-install tailwindcss -i rust/ui/styles/app.css -o public/app.css --minify)
