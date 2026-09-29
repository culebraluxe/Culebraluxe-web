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
    Node             24 (vercel-build-prod.sh refuses anything else)

## The commands, in the order you actually use them

1. **`pnpm build`** — `scripts/site-build.sh` (Yew UI release + assets) followed by
   `cargo build --release -p server --bin http`. The two artifacts the site is made of.

2. **`pnpm build:all`** — `scripts/build-all.sh`. Everything locally: `cargo check --workspace
   --all-targets`, the release server, `cargo test -p db -p server -p forge -p workflow`, then the
   WASM + Next build. **This is the pre-push verification path, not the deploy path** — the deploy
   does not run it and should not, because the server is built in a Linux container.

3. **`pnpm deploy:prod`** — `scripts/deploy-prod.sh`. **The clean path.** Five steps, each failing
   closed:
   1/5 build the stylesheet (`tailwindcss` over `rust/ui/styles/app.css`);
   2/5 compile locally with `docker build -f deploy/Dockerfile.build`, and refuse to continue unless
   both `culebraluxe` and `ui_bg.wasm` came out;
   3/5 pack the upload (`Dockerfile.runtime`, the binary and the wasm gzipped -9, `public/`, and
   `lib/forms/templates/`);
   4/5 `PATCH` the project to `framework=container` so it runs the application container;
   5/5 `vercel deploy --prod --yes` from the staged directory.
   Then it checks `/`, `/buyers`, `/app.css`, `/rust-ui/ui.js`, `/rust-ui/ui_bg.wasm`,
   `/api/rust-ui/public-page?screen=site-home` and `/login` all answer **200** on the canonical
   domain, and on any failure it prints the rollback: Vercel → culebraluxe-web-fp → Deployments →
   promote the previous one. A failed deploy leaves production unchanged.

4. **`pnpm release`** — `scripts/release-record.sh`. Build **and** deploy **and** probe **and
   record**, in one command, into the append-only `docs/agent/releases.md`.
   Flags: `--build` (build + record only), `--deploy` (deploy + probe + record), `--probe` (re-check
   the live SHA), `--last [N]`, `--verify <sha>`.
   **The rule that makes the record worth anything:** *a row is not a receipt.* Three named ways a
   receipt lies — a **stale cite** (a receipt for SHA A read as evidence for candidate B), a
   **partial row** (recorded with no live probe agreeing anything is serving), and a **last-line
   race** (reading a half-written final line). A receipt is ELIGIBLE only for the SHA it measured,
   only when build, deploy and a live probe all agree on that SHA. **A failed release is a row too**
   — a missing row is not evidence of a clean release.
   **The Forge chain never builds and never deploys.** It asks `pnpm release --verify <sha>` and
   reads the answer. DEV_OPS owns the receipt.

5. **`pnpm smoke:prod`** — the `smoke prod` subcommand of `rust/cli` (ported from `scripts/prod-smoke.ts`
   on 2026-09-28; the TypeScript file is deleted). Asks production whether it *works*, not whether
   it deployed: two checks, because a test suite answers "does the code work" and only the deployed
   artefact answers "does the thing people load work". Deliberate choices: the **canonical domain
   only** (a `*.vercel.app` URL answers 302 to Vercel Authentication — measured 2026-09-14 — so a
   check against it can never pass), and assertions against strings the pages own (title,
   "Selected Properties") rather than byte counts, so a copy edit is not a false alarm.
   `--expect-head` also asserts the live SHA equals HEAD; `--format json` for machines;
   `SMOKE_TIMEOUT_MS` for the clock.
   **Two of its six checks cannot pass today, and the port did not change that:** `/api/build-info`
   and `/api/rust-ready` answer **404** on production (verified by plain `curl` on 2026-09-28 — they
   are Next-app routes that went with the port, and their only surviving caller is this smoke). The
   other four checks pass. Either the Rust server grows those two routes (a build stamp and a
   readiness answer naming the database it resolved) or the smoke points at the routes that exist —
   the owner's call, because it is a production surface.

## Kept for build-only and deploy-only work

- `scripts/vercel-build-prod.sh` — the build half alone; refuses Node ≠ 24, pins the CLI.
- `scripts/vercel-deploy-prod.sh` — the deploy half alone; **must run from `main`**, refuses a
  changed tree, and exempts generated `docs/agent/manifest/*.md` rows because a release build
  rewrites their render timestamps by definition.
- `scripts/vercel-release-prod.sh` — build then deploy, with `main` + clean tree required up front.
- `scripts/rust-container-preflight.sh` — what the container needs before it will boot.
- `scripts/vercel-provision-rust-project.sh` — project provisioning (one-time).
- `pnpm container:dry-run` (`scripts/site-container.sh`), `pnpm scan:migrations`
  (`scripts/scan-migrations.sh`).

## The guards, stated once

1. Production deploys and releases run from **`main`** only. The scripts enforce it.
2. A **clean tree** is required (generated manifests excepted). An unrelated dirty file is a
   release blocker, not a detail.
3. Production deploys are a **captain's call**, and `deploy:prod` names the project, the team and
   the rollback path itself rather than trusting memory.
4. **A deploy is not a receipt and a receipt is not a deploy.** `deploy:prod` answering 200 is one
   fact; `pnpm release --verify <sha>` is the other. DEV_OPS cites the second.
5. Never `vercel --prod` by hand against `culebraluxe-web-fp`: it re-introduces the paid remote
   compile and bypasses the 200-check and the rollback line.
