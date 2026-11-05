#!/usr/bin/env bash
# Content-hash the three assets the shell names, so a deploy can never be handed the previous build's stylesheet,
# glue or wasm under a stable URL. Until this existed `/app.css`, `/rust-ui/ui.js` and `/rust-ui/ui_bg.wasm` kept
# their names on every deploy, which is why the edge cache rule was held at one hour: a long TTL would have served
# the old build's CSS against the new build's page after a deploy.
#
#   scripts/asset-hash.sh public          # any site dir holding app.css + rust-ui/{ui.js,ui_bg.wasm}
#
# For each asset that exists it writes a content-addressed copy beside the canonical name and records the mapping
# in `asset-manifest.json`:
#
#   app.css            -> app.<12 hex>.css
#   rust-ui/ui.js      -> rust-ui/ui.<12 hex>.js
#   rust-ui/ui_bg.wasm -> rust-ui/ui_bg.<12 hex>.wasm
#
# The canonical files are KEPT. An address from the machine that a rolling deploy has just replaced still resolves,
# because `web/src/site.rs` falls back to the canonical file when the exact hashed name is gone, and a developer
# build with no manifest keeps working exactly as before (the shell falls back to the canonical names).
#
# The deploy stage carries the wasm COMPRESSED (`ui_bg.wasm.gz`; devops/Dockerfile.runtime gunzips it), so a wasm
# with no plain file beside it is hashed through `gunzip -c` and materialized from the decompressed bytes.
set -euo pipefail

site="${1:-public}"
[ -d "$site" ] || { echo "asset-hash: no site directory at $site" >&2; exit 2; }

assets=(app.css rust-ui/ui.js rust-ui/ui_bg.wasm)
manifest="{"
first=1
for rel in "${assets[@]}"; do
  dir="$(dirname "$rel")"
  base="$(basename "$rel")"
  stem="${base%.*}"
  ext="${base##*.}"
  if [ "$dir" = "." ]; then
    hashed="$stem.HASH.$ext"
  else
    hashed="$dir/$stem.HASH.$ext"
  fi
  if [ -f "$site/$rel" ]; then
    hash="$(shasum -a 256 "$site/$rel" | cut -c1-12)"
    cp "$site/$rel" "$site/${hashed/HASH/$hash}"
  elif [ -f "$site/$rel.gz" ]; then
    hash="$(gunzip -c "$site/$rel.gz" | shasum -a 256 | cut -c1-12)"
    gunzip -c "$site/$rel.gz" > "$site/${hashed/HASH/$hash}"
  else
    continue
  fi
  hashed="${hashed/HASH/$hash}"
  [ "$first" = 1 ] || manifest="$manifest,"
  manifest="$manifest\"$rel\":\"$hashed\""
  first=0
  echo "asset-hash: $rel -> $hashed"
done
manifest="$manifest}"
printf '%s\n' "$manifest" > "$site/asset-manifest.json"
echo "asset-hash: $site/asset-manifest.json"
