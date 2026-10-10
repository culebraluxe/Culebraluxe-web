# DEV_OPS — the clean production compile, deploy and release set

This is the production path as it exists in the repository, written down so it does not have to be
rediscovered. Nothing here is new; every command already exists.

**The shape of it.** Everything is compiled **on this Mac, for free**, inside Docker
(`deploy/Dockerfile.build`): the Yew UI to WebAssembly, and the server cross-compiled for Vercel's
x86_64 Linux. Vercel only unpacks the finished files (`deploy/Dockerfile.runtime`), so a deploy costs
seconds instead of a paid ~14-minute Vercel compile. That is what "clean" means here — a local,
reproducible compile of exactly the bits that get uploaded.

Fixed facts, quoted from the scripts (do not retype them elsewhere):

    Vercel project   culebraluxe-web-fp   prj_RHzXYauXOgIh2abiEsMiQJ1V3jlV
    Vercel team      team_xk8vFaeSyY6CuSkS3OK55tTc
    Canonical site   https://www.culebraluxe.com
    Vercel CLI       59.25.4 (pinned, run through npx)
    Build path       `pnpm deploy:prod` (local Docker compile + Vercel container deploy)

## The commands, in the order you actually use them

1. **`pnpm build`** — `scripts/site-build.sh` (Yew UI release + assets) followed by
   `cargo build --release -p web --bin web`. The two artifacts the site is made of.

2. **`pnpm build:all`** — `scripts/build-all.sh`. Everything locally: `cargo check --workspace
   --all-targets`, the release server, `cargo test -p db -p web -p forge -p workflow`, then the
   WASM + Next build. **This is the pre-push verification path, not the deploy path** — the deploy
   does not run it and should not, because the server is built in a Linux container.

3. **`pnpm deploy:prod`** — `scripts/deploy-prod.sh`. **The clean path.** Five steps, each failing
   closed:
   1/5 build the stylesheet (`tailwindcss` over `web/ui/styles/app.css`);
   2/5 compile locally with `docker build -f deploy/Dockerfile.build`, and refuse to continue unless
   both `culebraluxe` and `ui_bg.wasm` came out;
   3/5 pack the upload (`Dockerfile.runtime`, the binary and the wasm gzipped -9, `public/`, and
   `middle/model/forms/templates/`);
   4/5 `PATCH` the project to `framework=container` so it runs the application container;
   5/5 `vercel deploy --prod --yes` from the staged directory.
   Then it checks `/`, `/buyers`, `/app.css`, `/rust-ui/ui.js`, `/rust-ui/ui_bg.wasm`,
   `/api/rust-ui/public-page?screen=site-home` and `/login` all answer **200** on the canonical
   domain, and on any failure it prints the rollback: Vercel → culebraluxe-web-fp → Deployments →
   promote the previous one. A failed deploy leaves production unchanged.

4. **`pnpm release`** — `scripts/release-record.sh`. Run the active production build/deploy **and** probe **and
   record**, in one command, into the append-only `docs/agent/releases.md`.
   Flags: `--deploy` (same active build + deploy + probe + record, without the informational CI lookup),
   `--probe` (re-check the live SHA), `--last [N]`, `--verify <sha>`. Build-only mode is retired because
   the old standalone build produced a different, obsolete frontend artifact.
   **The rule that makes the record worth anything:** *a row is not a receipt.* Three named ways a
   receipt lies — a **stale cite** (a receipt for SHA A read as evidence for candidate B), a
   **partial row** (recorded with no live probe agreeing anything is serving), and a **last-line
   race** (reading a half-written final line). A receipt is ELIGIBLE only for the SHA it measured,
   only when build, deploy and a live probe all agree on that SHA. **A failed release is a row too**
   — a missing row is not evidence of a clean release.
   **The Forge chain never builds and never deploys.** It asks `pnpm release --verify <sha>` and
   reads the answer. DEV_OPS owns the receipt.

5. **`pnpm smoke:prod`** — the `smoke prod` subcommand of `cli` (ported from `scripts/prod-smoke.ts`
   on 2026-09-28; the TypeScript file is deleted). Asks production whether it *works*, not whether
   it deployed: two checks, because a test suite answers "does the code work" and only the deployed
   artefact answers "does the thing people load work". Deliberate choices: the **canonical domain
   only** (a `*.vercel.app` URL answers 302 to Vercel Authentication — measured 2026-09-14 — so a
   check against it can never pass), and assertions against strings the pages own (title,
   "Selected Properties") rather than byte counts, so a copy edit is not a false alarm.
   `--expect-head` also asserts the live SHA equals HEAD; `--format json` for machines;
   `SMOKE_TIMEOUT_MS` for the clock.
   **Two of its six checks used to be impossible, and the Rust server now serves both** (2026-09-28,
   `bcc6e52e`): `/api/build-info` and `/api/rust-ready` were Next-app routes that went with the port,
   answering **404** on production (verified by plain `curl` on 2026-09-28), and the smoke was their only
   surviving caller. They are now routes of the Rust server: `/api/rust-ready` mounts the **same handler**
   as `/readyz`, so the container's own HEALTHCHECK and the release gate can never disagree, and
   `/api/build-info` serves the deployed commit from `CULEBRALUXE_BUILD_SHA`, which `deploy:prod` writes
   into the runtime image beside the build time. **`--expect-head` still fails until the next deploy** —
   the checks read the *live* build, so the stamp becomes real only once an image carries one. An
   unstamped or mis-stamped build answers with no sha and a note naming what to set, so the gate fails
   loudly rather than comparing against nothing. Readiness is `/api/rust-ready` (`ok` + `databaseTarget`)

## Retired release path

- `scripts/vercel-build-prod.sh`, `scripts/vercel-deploy-prod.sh`, and `scripts/vercel-release-prod.sh` belong to the
  retired prebuilt frontend path. They are not called by `pnpm release` and must not be used for production.
- `scripts/rust-container-preflight.sh` — what the container needs before it will boot.
- `scripts/vercel-provision-rust-project.sh` — project provisioning (one-time).
- `pnpm container:dry-run` (`scripts/site-container.sh`), `pnpm scan:migrations`
  (`scripts/scan-migrations.sh`).

## The guards, stated once

1. Production releases through `pnpm release` run from **`main`** only. The recorder enforces it.
2. A **clean tree** is required for recorded production releases. An unrelated dirty file is a
   release blocker, not a detail.
3. Production deploys are a **captain's call**, and `deploy:prod` names the project, the team and
   the rollback path itself rather than trusting memory.
4. **A deploy is not a receipt and a receipt is not a deploy.** `deploy:prod` answering 200 is one
   fact; `pnpm release --verify <sha>` is the other. DEV_OPS cites the second.
5. Never `vercel --prod` by hand against `culebraluxe-web-fp`: it re-introduces the paid remote
   compile and bypasses the 200-check and the rollback line.
