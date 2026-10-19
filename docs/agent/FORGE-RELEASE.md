# Forge release runbook

Forge is a **Mac/launchd worker against PROD Neon**, not a Vercel container. Its release shape mirrors the
other two deploys (local build, staged artifact, smoke, rollback) but the runtime target is the scheduler.

## Build

```bash
bash scripts/build-forge.sh            # tests + release build
bash scripts/build-forge.sh --skip-tests
pnpm forge:build
```

Produces `build/forge/<version>/` + `build/forge-<version>.tar.gz`:

- `bin/{forge,forge-worker,forge-task,forge-assay-plan-hash}`
- `definitions/FORGE_SDLC-v6.xml`
- `scripts/agent-worker-once.sh`
- `VERSION` + `MANIFEST.sha256`

`<version>` is the commit short SHA (override with `FORGE_BUILD_VERSION`). The lane's cargo target-dir
is resolved via `cargo metadata`, not assumed to be `target/release`.

## Deploy

```bash
bash scripts/deploy-forge-prod.sh                    # build + install + refresh launchd
bash scripts/deploy-forge-prod.sh --no-build --version <v>
bash scripts/deploy-forge-prod.sh --dry-run
pnpm forge:deploy
```

Steps: clean-tree check → build (unless `--no-build`) → install to
`~/Library/Application Support/CulebraLuxe/forge/<v>` → update `current`/`previous` symlinks → set
`AGENT_WORKER_BIN` in `.env.scheduler` (gitignored, mode 600) → `cli launchd agent-worker install` →
status. It **never runs a work pass**; the next 3-minute wake uses the released binary.

## Rollback

```bash
bash scripts/deploy-forge-prod.sh --rollback
```

Points `current` back at `previous`, rewrites the scheduler binary path, refreshes the wrapper.

## Scheduler behavior

`scripts/agent-worker-once.sh` uses `$AGENT_WORKER_BIN` when set (release artifact) and falls back to
`cargo run -p forge --bin forge-worker` for development. Missing/non-executable binary fails closed
with exit 2 and a log line; no stale compile on the wake path.

## Safety

- No PROD DB writes by build/deploy; the worker itself is unchanged.
- Deployment does not claim work in the script.
- Real deploy is an operator action (it changes the live scheduler); run the dry-run first.
