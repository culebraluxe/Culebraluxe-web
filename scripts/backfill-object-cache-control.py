#!/usr/bin/env python3
"""Backfill Cache-Control on existing Neon Object Storage objects.

Neon stores object metadata, not a bucket-wide cache policy. Older uploads have no Cache-Control,
so browsers refetch every image from the origin. This copies each object server-side with metadata
REPLACE, preserving bytes/content-type/sha256 and setting the same policy the app now writes:
  public media  -> public, max-age=31536000, immutable
  private media -> private, max-age=300
  vault         -> private, no-store
"""
import argparse, concurrent.futures, os
import boto3
from botocore.config import Config

POLICY = {
    "culebraluxe-property-media-public": "public, max-age=31536000, immutable",
    "culebraluxe-property-media-private": "private, max-age=300",
    "culebraluxe-vault": "private, no-store",
}


def client():
    return boto3.client(
        "s3",
        endpoint_url=os.environ["AWS_ENDPOINT_URL_S3"],
        region_name=os.environ.get("AWS_REGION", "us-east-2"),
        aws_access_key_id=os.environ["AWS_ACCESS_KEY_ID"],
        aws_secret_access_key=os.environ["AWS_SECRET_ACCESS_KEY"],
        config=Config(s3={"addressing_style": "path"}, signature_version="s3v4", retries={"max_attempts": 5, "mode": "standard"}),
    )


def rewrite(s3, bucket, key):
    head = s3.head_object(Bucket=bucket, Key=key)
    metadata = head.get("Metadata") or {}
    s3.copy_object(
        Bucket=bucket,
        Key=key,
        CopySource={"Bucket": bucket, "Key": key},
        MetadataDirective="REPLACE",
        CacheControl=POLICY[bucket],
        ContentType=head.get("ContentType", "application/octet-stream"),
        Metadata=metadata,
    )
    return key


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bucket", action="append", choices=list(POLICY), help="bucket(s); default all three")
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    s3 = client()
    buckets = args.bucket or list(POLICY)
    for bucket in buckets:
        keys = []
        paginator = s3.get_paginator("list_objects_v2")
        for page in paginator.paginate(Bucket=bucket):
            keys.extend(o["Key"] for o in page.get("Contents", []))
        print(f"{bucket}: {len(keys)} objects -> {POLICY[bucket]}")
        if args.dry_run:
            continue
        with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.workers)) as pool:
            futures = [pool.submit(rewrite, s3, bucket, key) for key in keys]
            done = 0
            for future in concurrent.futures.as_completed(futures):
                future.result()
                done += 1
                if done % 50 == 0:
                    print(f"  {bucket}: {done}/{len(keys)}")
        print(f"  {bucket}: done")


if __name__ == "__main__":
    main()
