#!/usr/bin/env bash
# Clone production (Postgres + Neon Object Storage) into a fresh dev branch.
#
# Neon branches are copy-on-write: a branch created from production inherits BOTH the database at
# fork time and every bucket/object in object storage. This script creates that branch, mints a
# branch-scoped storage credential, updates the local dev env, and (unless --no-promote) renames
# the old `dev` branch and promotes the clone to `dev`.
#
# The old dev branch is retained as `dev-retired-<stamp>` for rollback; nothing is deleted.
#
# Usage:
#   bash scripts/refresh-dev-from-prod.sh [--no-promote] [--env .env.local]
#
# Requires: authenticated neon CLI, python3, psql. Local-only; never points at production.
set -euo pipefail

PROJECT_ID="${NEON_PROJECT_ID:-snowy-salad-48970537}"
PROD_BRANCH="${NEON_PROD_BRANCH:-production}"
DEV_BRANCH="${NEON_DEV_BRANCH:-dev}"
ENV_FILE="${ENV_FILE:-.env.local}"
PROMOTE=1
STAMP="$(date -u +%Y%m%d-%H%M%S)"
NEW_NAME="dev-prod-clone-$STAMP"

while [ $# -gt 0 ]; do
  case "$1" in
    --no-promote) PROMOTE=0; shift ;;
    --env) ENV_FILE="${2:?--env needs a path}"; shift 2 ;;
    -h|--help) sed -n '1,30p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

command -v neon >/dev/null || { echo "neon CLI is required" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 is required" >&2; exit 1; }
command -v psql >/dev/null || { echo "psql is required" >&2; exit 1; }
[ -f "$ENV_FILE" ] || { echo "env file not found: $ENV_FILE" >&2; exit 1; }

json() { python3 -c 'import json,sys; d=json.load(sys.stdin); print(eval(sys.argv[1], {"d": d}))' "$1"; }

echo "==> creating branch $NEW_NAME from $PROD_BRANCH"
neon branches create --project-id "$PROJECT_ID" --name "$NEW_NAME" --parent "$PROD_BRANCH" -o json > /tmp/neon-refresh-branch.json
NEW_ID="$(json 'd.get("branch",d).get("id")' < /tmp/neon-refresh-branch.json)"
[ -n "$NEW_ID" ] || { echo "branch id missing" >&2; exit 1; }

echo "==> waiting for $NEW_NAME ($NEW_ID) to be ready"
for _ in $(seq 1 60); do
  state="$(neon branches list --project-id "$PROJECT_ID" -o json | python3 -c 'import json,sys; d=json.load(sys.stdin); print(next((b.get("current_state","") for b in d if b.get("id")==sys.argv[1]),""))' "$NEW_ID")"
  [ "$state" = "ready" ] && break
  sleep 5
done
[ "${state:-}" = "ready" ] || { echo "branch did not become ready (state=$state)" >&2; exit 1; }

echo "==> object storage inherited?"
BUCKETS="$(neon buckets list --project-id "$PROJECT_ID" --branch "$NEW_ID" -o json)"
echo "$BUCKETS" | python3 -c 'import json,sys; d=json.load(sys.stdin); b=d.get("buckets",d) if isinstance(d,dict) else d; print("buckets:", ", ".join(x.get("name","?") for x in b))'
for bucket in culebraluxe-property-media-public culebraluxe-property-media-private culebraluxe-vault; do
  n="$(neon buckets object list "$bucket" --project-id "$PROJECT_ID" --branch "$NEW_ID" --recursive -o json 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); rows=d.get("objects",d) if isinstance(d,dict) else d; print(len(rows))' 2>/dev/null || echo 0)"
  echo "  $bucket objects=$n"
done

echo "==> minting branch-scoped storage credential"
neon api "/projects/$PROJECT_ID/branches/$NEW_ID/credentials" -X POST \
  -d "{\"scopes\":[\"storage:read\",\"storage:write\"],\"principal_type\":\"user\",\"name\":\"culebraluxe-media-dev-$STAMP\"}" \
  -o json > /tmp/neon-refresh-credential.json
chmod 600 /tmp/neon-refresh-credential.json
neon api "/projects/$PROJECT_ID/branches/$NEW_ID/storage" -o json > /tmp/neon-refresh-storage.json

echo "==> fetching connection strings"
POOLED_URL="$(neon connection-string "$NEW_NAME" --project-id "$PROJECT_ID" --pooled)"
UNPOOLED_URL="$(neon connection-string "$NEW_NAME" --project-id "$PROJECT_ID")"

echo "==> updating $ENV_FILE (backup kept)"
cp -p "$ENV_FILE" "$ENV_FILE.bak-$STAMP"
python3 - "$ENV_FILE" /tmp/neon-refresh-credential.json /tmp/neon-refresh-storage.json "$POOLED_URL" "$UNPOOLED_URL" <<'PY'
import json, os, re, sys
path, cred_path, storage_path, pooled, unpooled = sys.argv[1:6]
cred = json.load(open(cred_path)); storage = json.load(open(storage_path))
updates = {
    "DATABASE_URL_DEV": pooled,
    "DATABASE_URL": pooled,
    "DATABASE_URL_UNPOOLED": unpooled,
    "MEDIA_OBJECT_STORE_ENDPOINT": storage["s3_endpoint"],
    "MEDIA_OBJECT_STORE_REGION": storage["region"],
    "MEDIA_OBJECT_STORE_ACCESS_KEY_ID": cred["token_id"],
    "MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY": cred["s3_secret_access_key"],
    "MEDIA_BUCKET_PROPERTY_PUBLIC": "culebraluxe-property-media-public",
    "MEDIA_BUCKET_PROPERTY_PRIVATE": "culebraluxe-property-media-private",
    "MEDIA_BUCKET_VAULT": "culebraluxe-vault",
}
lines = open(path).read().splitlines()
seen = set()
out = []
for line in lines:
    m = re.match(r"^([A-Z0-9_]+)=", line)
    if m and m.group(1) in updates:
        out.append(f"{m.group(1)}={updates[m.group(1)]}")
        seen.add(m.group(1))
    else:
        out.append(line)
for key, value in updates.items():
    if key not in seen:
        out.append(f"{key}={value}")
open(path, "w").write("\n".join(out) + "\n")
os.chmod(path, 0o600)
print("updated keys:", ", ".join(sorted(updates)))
PY

# Read one value with Python; shell-sourcing this env file is unsafe because provider secrets
# contain `$` and unquoted URLs contain `&`.
env_value() {
  python3 - "$ENV_FILE" "$1" <<'PYENV'
import sys
path, key = sys.argv[1:3]
for raw in open(path, errors="replace"):
    line = raw.strip()
    if not line or line.startswith("#") or "=" not in line:
        continue
    k, v = line.split("=", 1)
    if k.strip() == key:
        v = v.strip()
        if len(v) >= 2 and v[0] == v[-1] and v[0] in "\"'":
            v = v[1:-1]
        print(v)
        break
PYENV
}
DEV_URL="$(env_value DATABASE_URL_DEV)"
echo "==> verifying clone"
psql "$DEV_URL" -Atc "select 'media='||count(*)||' verified='||count(*) filter (where storage_status='VERIFIED') from media;" \
  -c "select 'file_data_column='||count(*) from information_schema.columns where table_name='media' and column_name='file_data';"
if [ "$PROMOTE" = "1" ]; then
  echo "==> promoting $NEW_NAME to $DEV_BRANCH (old branch kept as $DEV_BRANCH-retired-$STAMP)"
  if neon branches list --project-id "$PROJECT_ID" -o json | python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if any(b.get("name")==sys.argv[1] for b in d) else 1)' "$DEV_BRANCH"; then
    neon branches rename "$DEV_BRANCH" "$DEV_BRANCH-retired-$STAMP" --project-id "$PROJECT_ID" >/dev/null
  fi
  neon branches rename "$NEW_NAME" "$DEV_BRANCH" --project-id "$PROJECT_ID" >/dev/null
fi

echo "==> done: $NEW_NAME ($NEW_ID); env=$ENV_FILE; backup=$ENV_FILE.bak-$STAMP"
echo "    old dev branch retained as $DEV_BRANCH-retired-$STAMP"
