# Machine layout: where the code lives, and why

Written 2026-10-01, when the tree moved out of `~/Documents`. This is the machine half of the handbook —
`AGENTS.md` says how to work, this says where.

## The tree

```
/Users/Shared/dev/
├── src/
│   ├── Culebraluxe-web/     main checkout,   branch main        (the repo you are in)
│   ├── lane-gpt/            git worktree,    branch lane/gpt
│   ├── lane-claude/         git worktree,    branch lane/claude
│   └── lane-deep/           git worktree,    branch lane/deep
└── build/
    ├── rust/                CARGO_TARGET_DIR, shared by every worktree
    └── logs/                launchd job logs (wip-snapshot.log, wip-snapshot.err.log)
```

Nothing else belongs in `src/`: no exports, no caches, no second copies of the repo.

## Lanes

```sh
cd /Users/Shared/dev/src/Culebraluxe-web
git worktree add ../lane-<name> -b lane/<name> origin/main
cd ../lane-<name> && pnpm install
```

One lane, one branch, one agent: commit on the lane branch, land it on `main` from the main checkout, and mind house
rule 1 (`lane/*` is short-lived). Lanes are worktrees, so they share history and one object store — that is the point.
`CARGO_TARGET_DIR` is shared too, which is why four checkouts do not cost four 34 GB targets; the cost is that two
simultaneous `cargo` runs serialize on the target lock instead of running in parallel. Lane checkout size on disk is
~84 MB of source plus pnpm's hardlinked `node_modules`.

## Rules that keep this working

1. **Never work in `/tmp` or `~/Documents`.** macOS purges `/tmp`, and iCloud resurrects deletions in `~/Documents`;
   both leave dead `git worktree` records behind, and a dead record makes `git worktree list` lie about what work exists.
2. **No stores inside a checkout.** pnpm's store is `~/Library/pnpm/store/v10`; a `.pnpm-store` inside the repo is a
   leftover from an older config (one was removed on 2026-10-01, 936 MB, referenced by no `.npmrc`).
3. **`700`, on purpose.** A second account (`cecochran`) exists on this Mac and `/Users/Shared` is world-readable, so
   the tree, the lanes and every `.env*` file are owner-only. Do not loosen the modes.
4. **Never run a command that pages or waits for an editor.** `core.pager` is `cat` machine-wide for a reason — see below.
5. **Uncommitted work is snapshotted, not lost.** `scripts/wip-snapshot.sh` writes every dirty worktree to
   `refs/wip/<name>` every 5 minutes (launchd, `pnpm wip:install` / `wip:now` / `wip:uninstall`):

   ```sh
   git --no-pager diff HEAD refs/wip/lane-gpt --stat    # what the snapshot holds
   git show refs/wip/lane-gpt:path/to/file              # a specific file
   ```

## Why here

`~/Documents` is both iCloud-synced and TCC-protected. Three things followed from that, all observed rather than
theorised: every git command could raise a privacy prompt and stall a worker; dead `target` directories were being
synced to iCloud (43 GB across the old worktrees); and half-deleted worktrees left records that pointed at nothing.
`/Users/Shared` is outside TCC and outside iCloud, so none of it can happen again — the price is rule 3.

The other trap was the pager: eight `git` commands were found on 2026-10-01 stuck in `less` for 6–26 hours, waiting for
a `q` no agent can press, which is what a "blocked" agent looks like from the outside. Hence `core.pager=cat` and rule 4.

## Open items on this machine (not yet done as of 2026-10-01)

- Four LaunchAgents still point at the dead `~/Documents/Culebraluxe-web` path: `apple-local-listener`, `agent-worker`,
  `apple-sync`, `calendar-sync` (`~/Library/LaunchAgents/com.culebraluxe.*`). They should be repointed here.
- **No Time Machine destination.** The repo, four branches and the exports live on one internal disk; iCloud is not a
  backup, and it is currently holding ~43 GB of dead targets (see next line). This is the biggest open risk.
- Still registered as worktrees, all removable: `/private/tmp/ocwt` (87 MB, in a purgeable directory) and
  `~/Documents/Culebraluxe-web-roles` (33 GB, clean, HEAD `30b53b5d` — present in main). A third copy,
  `~/Documents/Culebraluxe-web-claude` (9.6 GB), is an orphan: its `.git` file points at
  `~/Documents/Culebraluxe-web/.git/worktrees/…`, which no longer exists.
- `build/rust` is still empty, so the next `cargo` invocation pays the full cold compile. Warming it is optional.

## The layout move (2026-10-01): what a lane does

The three lane worktrees sit on `97785410`, which is before the tree moved to the three tiers. Do not merge `main`
into a lane to catch up: the merge is a conflict in every file that moved, and the tree that comes out of it is
neither layout. Replay it instead — the script is the move, written so it checks before it acts and running it twice
does nothing.

```sh
cd /Users/Shared/dev/src/lane-<name>

# 1. the two scripts, from main (the lane's own commit predates them)
git checkout main -- scripts/restructure-domain-layout.sh scripts/validate-move-script.sh

# 2. does every rule still apply to THIS tree? 0 stale rules, or the report names the ones that moved under you
MOVE_BASE=97785410 bash scripts/validate-move-script.sh

# 3. the move (git mv throughout, so history follows each file)
bash scripts/restructure-domain-layout.sh

# 4. it compiles, or it did not happen
cargo check --workspace --all-targets
cargo test --workspace
```

Measured on 2026-10-01 in a rehearsal worktree at `97785410`: **51 rules apply, 0 stale**, the script exits 0, the tree
it produces holds 90 case files in `tests/tests/` with `rust/` and `deploy/` gone, and
`cargo check --workspace --all-targets` finishes with **0 errors**.

Two things the script deliberately does NOT do. It does not resolve the conflict between a lane's own work and the
layout — it moves the crate directories and re-spells the paths, and a lane's edits inside a moved file travel with it
because every step is a `git mv`. And it does not run `cargo fmt`: four module trees are deferred on purpose (a live
agent owns them), and a sweep that formats everything would collide with that agent's edits. Format the files you
touched, as always.

