# Backup and Restore Runbook

Status: current as of 2026-10-22  
System: CulebraLuxe web (Vercel container + Neon Postgres + Neon Object Storage)  
Audience: whoever is about to run a destructive migration or needs to prove recovery works.

## The model

Neon branches are copy-on-write for **both** Postgres and Object Storage:

- A branch created from `production` inherits the database at that instant.
- It also inherits every bucket and object that existed at that instant.
- Nothing is copied upfront; later writes/deletes on either branch are isolated.
- There is **no separate object-store backup key or snapshot job**. The branch is the backup.

Consequences:

- A backup is only as new as the moment the branch was created.
- Changes made after the fork are **not** in that backup.
- Deleting or expiring a branch deletes that recovery point.
- A database-only restore is not enough: the media rows carry object keys, so the object branch must restore with them.

## Current production identifiers

Keep these in the runbook, not in application code:

```text
Neon project:   snowy-salad-48970537
Production:     production        br-snowy-fog-axg3jae2
Development:    dev               br-withered-thunder-ax04r1s8
Object buckets: culebraluxe-property-media-public
                culebraluxe-property-media-private
                culebraluxe-vault
```

Object Storage is S3-compatible and path-style, in `us-east-2`.
Production public media is served through Vercel at `/media-cdn/...`; the
destination is hardcoded in `vercel.json` and points at the production branch
endpoint.

## Known state

As of the 2026-10-22 restore drill:

- `prod-blob-precleanup-20261021` is **no longer present** in the project. The
  project currently lists only `production`, `dev`, `forgevcs-ci`, and
  `dev-retired-20261021-074956`.
- There is therefore **no retained pre-cutover rollback branch**.
- The current production state restores correctly (see the drill record below).

Create a fresh retained branch before the next destructive change, and do not
assume an old snapshot still exists without listing it.

## Create a backup branch

Run before any migration that drops a column, drops a table, rewrites object
keys, or otherwise changes production irreversibly.

```bash
PROJECT=snowy-salad-48970537
STAMP=$(date -u +%Y%m%d-%H%M%S)
NAME="prod-safe-$STAMP"

neon branches create \
  --project-id "$PROJECT" \
  --name "$NAME" \
  --parent production \
  -o json | tee "/tmp/$NAME.json"

# Record and verify the branch id/state.
neon branches list --project-id "$PROJECT" -o table

# Optional: set an explicit expiration only if you have a retention policy
# that wants it. Do NOT set one on the recovery point you intend to keep.
# neon branches set-expiration "$NAME" --project-id "$PROJECT"
```

Checklist:

- [ ] Branch state is `ready`.
- [ ] Branch name records the date and “safe”/“pre-<change>”.
- [ ] Branch id is recorded in the change ticket/run log.
- [ ] Confirm the bucket list on the branch is non-empty before calling it a
      backup (see the drill below).
- [ ] Leave production alone.

## Run a restore drill

The drill creates an isolated copy, reads it, and deletes it. It never writes to
production.

### 1. Create the drill branch

```bash
PROJECT=snowy-salad-48970537
DRILL="restore-drill-$(date -u +%Y%m%d-%H%M%S)"

neon branches create \
  --project-id "$PROJECT" \
  --name "$DRILL" \
  --parent production \
  -o json > /tmp/drill-branch.json

# Wait until state=ready, then record the branch id.
neon branches list --project-id "$PROJECT" -o json
```

For a real recovery test, use the backup branch as `--parent` instead of
`production`.

### 2. Verify the database copy

```bash
DB_URL=$(neon connection-string "$DRILL" --project-id "$PROJECT")
echo "$DB_URL" > /tmp/drill-db.txt

psql "$DB_URL" -Atc "
  select 'media=' || count(*)
       || ' verified=' || count(*) filter (where storage_status = 'VERIFIED')
    from media;"

psql "$DB_URL" -Atc "
  select derivative_kind, count(*)
    from media
   where derivative_kind is not null
   group by derivative_kind
   order by derivative_kind;"

psql "$DB_URL" -Atc "
  select object_bucket, visibility, count(*)
    from media
   where object_key is not null
   group by object_bucket, visibility
   order by object_bucket, visibility;"
```

A usable current production copy should show the image originals plus the
`card`/`web`/`thumb` derivatives. Counts grow as new media is uploaded, so do not
treat a number as fixed; check that the counts are plausible and that
`VERIFIED` matches the object-key rows.

### 3. Mint a read-only storage credential for the drill branch

Credentials are branch-scoped. Never use production media credentials for a
drill branch.

```bash
DRILL_ID=<branch id from step 1>
neon api \
  "/projects/$PROJECT/branches/$DRILL_ID/credentials" \
  -X POST \
  -d '{"scopes":["storage:read"],"principal_type":"user","name":"restore-drill-read"}' \
  -o json > /tmp/drill-cred.json

neon api \
  "/projects/$PROJECT/branches/$DRILL_ID/storage" \
  -o json > /tmp/drill-storage.json
```

Put only those values in a temporary env file (mode 600), e.g.:

```bash
# /tmp/drill-media.env, generated from the two JSON files
AWS_ENDPOINT_URL_S3=...
AWS_REGION=us-east-2
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...
```

### 4. Verify object storage

List the three buckets and confirm the public media pointers resolve:

```python
# /tmp/drill-objects.py
import boto3, hashlib, os
from botocore.config import Config

s3 = boto3.client(
    "s3",
    endpoint_url=os.environ["AWS_ENDPOINT_URL_S3"],
    region_name=os.environ.get("AWS_REGION", "us-east-2"),
    aws_access_key_id=os.environ["AWS_ACCESS_KEY_ID"],
    aws_secret_access_key=os.environ["AWS_SECRET_ACCESS_KEY"],
    config=Config(s3={"addressing_style": "path"}),
)

for bucket in [
    "culebraluxe-property-media-public",
    "culebraluxe-property-media-private",
    "culebraluxe-vault",
]:
    n = total = 0
    for page in s3.get_paginator("list_objects_v2").paginate(Bucket=bucket):
        for obj in page.get("Contents", []):
            n += 1
            total += obj["Size"]
    print(bucket, n, "objects", total, "bytes")
```

Then export the public rows from the drill DB and compare keys + sizes:

```bash
psql "$DB_URL" -AtF $'\t' -c "
  select object_key, object_sha256, object_size
    from media
   where object_bucket = 'property_media_public'
     and object_key is not null
     and storage_status = 'VERIFIED';" > /tmp/drill-public-rows.tsv
```

Check:

- every DB `object_key` exists in the bucket;
- bucket size equals `object_size`;
- a sample object's SHA-256 equals `object_sha256`;
- the object count is at least the number of public DB rows.

A simple public-object check that needs no credentials at all:

```bash
EP=$(python3 -c "import json;print(json.load(open('/tmp/drill-storage.json'))['s3_endpoint'])")
KEY=$(head -1 /tmp/drill-public-rows.tsv | cut -f1)
curl -sS "$EP/culebraluxe-property-media-public/$KEY" -o /tmp/drill-object
shasum -a 256 /tmp/drill-object
```

### 5. Clean up

Always delete the drill branch and credential:

```bash
CRED_ID=$(python3 -c "import json;d=json.load(open('/tmp/drill-cred.json'));print(d.get('id') or d.get('token_id'))")
neon api \
  "/projects/$PROJECT/branches/$DRILL_ID/credentials/$CRED_ID" \
  -X DELETE -o json

neon branches delete "$DRILL_ID" --project-id "$PROJECT"
rm -f /tmp/drill-*.json /tmp/drill-*.env /tmp/drill-*.txt /tmp/drill-*.tsv
```

Production is untouched throughout.

## Restore for real

Do not point production at a restored branch until the restored copy has been
verified with the drill above.

1. Create a restore branch from the chosen backup branch.
2. Attach/verify compute and get its connection string.
3. Mint branch-scoped storage credentials.
4. Update the deployment environment:
   - `DATABASE_URL`, `DATABASE_URL_DEV`/`DATABASE_URL_PROD` as appropriate;
   - `MEDIA_OBJECT_STORE_ENDPOINT`;
   - `MEDIA_OBJECT_STORE_ACCESS_KEY_ID` / `MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY`;
   - `MEDIA_OBJECT_STORE_REGION`;
   - `MEDIA_BUCKET_PROPERTY_PUBLIC`, `MEDIA_BUCKET_PROPERTY_PRIVATE`,
     `MEDIA_BUCKET_VAULT`;
   - `MEDIA_PUBLIC_CDN_BASE` if the public site base changes.
5. **Update the `/media-cdn/` destination in `vercel.json`.** It currently
   hardcodes the production branch endpoint:
   `https://br-snowy-fog-axg3jae2.storage.c-4.us-east-2.aws.neon.tech/...`.
   If the restored branch has a different endpoint, public image URLs will
   otherwise still resolve to the old branch.
6. Redeploy and smoke test: home, health, a property gallery, a public image
   through `/media-cdn`, an authenticated PDF, and a PDF denial for anonymous
   access.
7. Keep the old production branch until the restored deployment is verified;
   then retire it deliberately.

## Latest drill record (2026-10-22)

Temporary branch: `restore-drill-20261022-064606` (created from `production`,
deleted after the drill; branch id `br-steep-credit-axy6351m`).

```text
media rows:        880
verified:          879
images:            784
card/web/thumb:    196 / 196 / 196
public bucket:     464 objects, 521,897,765 bytes
private bucket:    740 objects, 485,171,434 bytes
vault bucket:       95 objects,  50,935,420 bytes
public DB rows:    464
missing objects:     0
size mismatches:     0
sample SHA-256:     matched
```

The drill passed. The temporary branch and read-only credential were deleted.
