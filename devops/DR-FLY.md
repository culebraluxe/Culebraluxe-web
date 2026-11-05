# Production runbook — CulebraLuxe on Fly

**Production is `culebraluxe-prod` on Fly.io**, two machines in Ashburn; `culebraluxe.com` and
`www.culebraluxe.com` point at its dedicated IPv6. The standby is `culebraluxe-dr` (Chicago, one sleeping machine)
and `devops/FAILOVER.md` is the switch. Vercel is retired: its deploy path lives in `devops/break-glass/vercel/`
and is not used. The parent plan and its history: `devops/VERCEL-EXIT.md`.

The one-command version: `scripts/deploy-prod.sh` (a shim) → `scripts/deploy-fly.sh`.

## What production is

| | |
|---|---|
| Fly app | `culebraluxe-prod`, region `iad` (Ashburn), **two** machines, `shared-cpu-1x` / 512 MB |
| Standby | `culebraluxe-dr`, region `ord` (Chicago), one machine that sleeps — `devops/fly.dr.toml` |
| Image | built on this Mac (`devops/Dockerfile.build`), pushed with `crane`, run by `fly deploy --image` — Fly never builds |
| Config | `devops/fly.toml` (production) and `devops/fly.dr.toml` (standby); no `[build]` section on purpose |
| Database | Neon **production** branch — the app resolves it from `APP_ENV` (see the trap below) |
| Source of the code | Scion `culebraluxe-web@main` by default; `public/` comes from this checkout |
| DNS | Cloudflare zone `culebraluxe.com`; the cutover from Vercel is Claude + Lisa's job |

## Deploy

```sh
scripts/deploy-prod.sh                  # = scripts/deploy-fly.sh, source = Scion (default)
scripts/deploy-fly.sh --source local    # build from this checkout instead (use when ForgeVCS is down)
scripts/deploy-fly.sh --resume <tag>    # skip the build: push + deploy an image already built here
scripts/deploy-fly.sh --no-deploy       # build and push, do not release
```

The script builds the wasm UI and the server for linux/amd64, packs the runtime stage, content-hashes the
assets (`scripts/asset-hash.sh`), pushes with `crane` (resumable; `docker push` as fallback), and deploys
**bluegreen** — the new machine must answer `/healthz` before the old one is removed, so a bad release
leaves the previous version serving. Only after production's smoke passes does the script update the standby
(`culebraluxe-dr`) to the same image, so a DR site is never a build that failed its own smoke. `--no-dr` skips it.

A run is only a success when the smoke says so. It proves:

- `/api/build-info` reports the `builtAt` this run stamped — the machine is this image, not an older one;
- when the build came from Scion, `/api/build-info` reports the same `changeset` this build exported;
- `databaseTarget` is `prod` — a machine booted against DEV is a failed release, not a partial one;
- `/healthz`, `/`, `/buyers`, `/app.css`, `/rust-ui/ui.js`, `/rust-ui/ui_bg.wasm`, the public page and `/login`
  all answer 200;
- `scripts/smoke-site.sh` integration checks (photos, MUX, WhatsApp, LuxeSign, settings).

## Verify by hand

```sh
curl -s https://culebraluxe-prod.fly.dev/healthz
curl -s https://culebraluxe-prod.fly.dev/api/build-info   # changeset / builtAt / databaseTarget
scripts/smoke-site.sh                                   # the 20-check integration sweep
```

## Rollback

Every deploy prints the previous complete release in its REPORT. Rolling back is another `fly deploy` of
that image — no rebuild, no source:

```sh
fly releases --app culebraluxe-prod --image        # status `complete` rows only
fly deploy --app culebraluxe-prod --config devops/fly.toml \
  --image registry.fly.io/culebraluxe-dr:<previous-tag> --strategy bluegreen --wait-timeout 300
```

If the release never became healthy, the previous machine is still running; the deploy report names the
image to go back to and the machine list shows the leftover:
`fly machine list --app culebraluxe-prod`.

## Secrets

They live on the Fly app, never in the repo:

```sh
fly secrets list --app culebraluxe-prod         # names and digests only, never values
fly secrets set --stage --app culebraluxe-prod NAME=value
fly secrets import --stage --app culebraluxe-prod < file-only-in-a-600-file
```

**THE TRAP: `APP_ENV` declares the database target, and `VERCEL_ENV` is no longer read.** The app used to
prefer `VERCEL_ENV` (which Vercel set to `production`), which masked the fact that `APP_ENV` on this app is
not `production`. `db::resolve_declared_target` now reads `APP_ENV` only, so:

- `APP_ENV` **must** be `production` on `culebraluxe-prod` (and on the standby) — otherwise the app resolves DEV and refuses to
  boot on a schema mismatch (`db.boot_gate`, "N of N required migration(s) are unapplied on dev");
- `VERCEL_ENV`, if still present, is ignored and should be unset.

## Dependencies production needs

- **Neon production branch** (`br-snowy-fog-axg3jae2`): database and object storage. The app fails its boot
  gate rather than serving a schema it does not match.
- **ForgeVCS** (`culebraluxe-forgevcs` on Fly, production Neon branch): the source of the code a default
  deploy builds. If it is down, `--source local`.
- **Google OAuth**: the redirect URI for the deployed host must be registered.
- **Cloudflare**: zone records, SSL Full (strict), and the cache rule. Hashed assets are immutable for a
  year at the origin; the stable aliases are five minutes; HTML is `s-maxage=300` with
  `stale-while-revalidate=86400`; everything personal is `private, no-store`.

## Break glass

`devops/break-glass/vercel/` holds the retired Vercel deploy, verbatim, with a README. It is kept as an
emergency exit at the Captain's request. Its environment no longer exists (Sensitive env vars, reissued
storage keys), so expect to rebuild that before it could deploy anything.
