# Break glass — the Vercel deploy path

**Not a deploy path. Production is Fly.** `scripts/deploy-prod.sh` and `scripts/deploy-fly.sh` deploy
`culebraluxe-dr`; nothing in the normal release reads this directory.

These files are kept because the Captain asked for the Vercel path to survive as an emergency exit
(2026-11-05), the same way the retired TypeScript under `legacy/` is marked rather than deleted. They
are the last state of a path that shipped production from 2025 until the Fly cutover.

## What is here

- `vercel-deploy-prod.sh`, `vercel-build-prod.sh`, `vercel-release-prod.sh`,
  `vercel-provision-rust-project.sh` — the Vercel deploy, build, release and project-provisioning
  scripts, moved verbatim.
- `Dockerfile.vercel`, `Dockerfile.vercel.rust-api` — the Vercel runtime images.
- `vercel.json` — rewrites (including the `/media-cdn/:path*` rule that served photos), functions
  config and the deployment-enabled flag.
- `vercelignore` — was `.vercelignore` at the root; the CLI only reads it at the root, so an
  emergency use copies it back.

## If you actually need it

The environment this expects is gone: the Vercel project's production variables were Sensitive
(write-only), several were `[SENSITIVE]` placeholders at the end, and the Neon storage credentials
it used have since been reissued for the Fly app. Expect to rebuild all of that from the Neon console
before the first deploy. Treat this as a design reference — what the Vercel deployment had to
configure — more than a button that still works.

## Why it is not deleted

`AGENTS.md` rule: dead code is marked, not deleted when it records how something worked. The Vercel
`/media-cdn/:path*` rewrite is the reason `MEDIA_PUBLIC_CDN_BASE` exists at all, and the two
`Dockerfile.vercel` files show the build inputs the Fly runtime image inherited.
