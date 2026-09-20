#!/usr/bin/env bash
# CulebraLuxe-owned vendor pin tools. Never run upstream install.sh.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
VENDOR="$ROOT/vendor/council-of-high-intelligence"
PIN="$ROOT/vendor/council-pin.json"
CMD="${1:-status}"
TAG="${2:-}"

need_vendor() {
  if [[ ! -d "$VENDOR/.git" && ! -f "$VENDOR/.git" ]]; then
    echo "vendor checkout missing. From repo root:"
    echo "  git submodule update --init vendor/council-of-high-intelligence"
    exit 1
  fi
}

pin_sha() {
  python3 - "$PIN" <<'PY'
import json,sys
print(json.load(open(sys.argv[1]))["sha"])
PY
}

pin_tag() {
  python3 - "$PIN" <<'PY'
import json,sys
print(json.load(open(sys.argv[1])).get("tag") or "")
PY
}

checkout_sha() {
  git -C "$VENDOR" rev-parse HEAD
}

case "$CMD" in
  status)
    need_vendor
    echo "pin tag:      $(pin_tag)"
    echo "pin sha:      $(pin_sha)"
    echo "checkout sha: $(checkout_sha)"
    git -C "$VENDOR" fetch --tags --quiet || true
    echo "origin tags (last 5):"
    git -C "$VENDOR" tag --sort=-v:refname | head -5
    if [[ "$(pin_sha)" != "$(checkout_sha)" ]]; then
      echo "DRIFT: checkout is not the pin. CouncilService will refuse to convene."
      exit 2
    fi
    echo "ok"
    ;;
  fetch)
    need_vendor
    git -C "$VENDOR" fetch --tags
    ;;
  propose)
    need_vendor
    if [[ -z "$TAG" ]]; then
      echo "usage: $0 propose <tag>"
      exit 1
    fi
    git -C "$VENDOR" fetch --tags
    git -C "$VENDOR" checkout --detach "$TAG"
    echo "proposed $(git -C "$VENDOR" rev-parse HEAD) ($TAG)"
    echo "diff protocol files against previous pin:"
    git -C "$VENDOR" diff "$(pin_sha)" -- SKILL.md agents || true
    echo "If adapter tests pass: $0 pin && commit pin + submodule SHA together."
    ;;
  pin)
    need_vendor
    SHA="$(checkout_sha)"
    NEW_TAG="$(git -C "$VENDOR" describe --tags --exact-match 2>/dev/null || pin_tag)"
    python3 - "$PIN" "$SHA" "$NEW_TAG" <<'PY'
import json,sys
path, sha, tag = sys.argv[1], sys.argv[2], sys.argv[3]
data = json.load(open(path))
data["sha"] = sha
data["tag"] = tag or data.get("tag")
json.dump(data, open(path,"w"), indent=2)
print("pinned", sha, data.get("tag"))
PY
    ;;
  verify)
    need_vendor
    test -f "$VENDOR/SKILL.md"
    test -d "$VENDOR/agents"
    test "$(pin_sha)" = "$(checkout_sha)"
    echo "verify ok"
    ;;
  *)
    echo "usage: $0 status|fetch|propose <tag>|pin|verify"
    exit 1
    ;;
esac
