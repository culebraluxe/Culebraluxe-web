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

One lane, one branch, one agent: commit on the lane branch, land it on `origin/main`, and mind house rule 1
(`lane/*` is short-lived). **`origin/main` is the trunk, not the main checkout's local `main`:** Forge's publish path
(`forge/src/engine/git_publish.rs`) pushes candidates straight to `origin/main`, so a local `main` is always behind
something. Check out by rebasing (`git fetch origin && git rebase origin/main` in the lane), never by merging `main`
in; check in with `git push origin lane/<name>:main`, which is fast-forward-only — a lane that has not rebased is
refused instead of merged. Nobody commits on the main checkout; it follows the trunk with `git pull --ff-only`.
No lane reads another lane's tree — work crosses lanes only through `main` (`AGENTS.md`,
the 2026-10-03 lane exception to NO TREES). Lanes are worktrees, so they share history and one object store — that is the point.
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

## Open items on this machine (the 2026-10-01 list, re-checked 2026-10-03)

Three of the four below closed on 2026-10-02/03. They are kept with the date rather than deleted, because the
2026-10-01 wording described a machine that no longer exists and a reader who trusts it re-does finished work.

- **The LaunchAgents are repointed (fixed).** All five — `agent-worker`, `apple-sync`, `apple-local-listener`,
  `calendar-sync`, `wip-snapshot` (`~/Library/LaunchAgents/com.culebraluxe.*`) — carry
  `/Users/Shared/dev/src/Culebraluxe-web`: `agent-worker` as `AGENT_WORKER_REPO`, the Apple jobs as `CULEBRALUXE_REPO`,
  each regenerated from its own template by its own `pnpm *:install`. `plutil -p` finds no live path naming
  `~/Documents`; the only mentions left are in the templates' comments.
  **What is NOT settled:** `launchctl list` on 2026-10-03 shows `agent-worker`, `apple-sync` and `calendar-sync` with
  PID `-` and last exit status 1, `apple-local-listener` running (pid 78), `wip-snapshot` exiting 0. Exit 1 is not by
  itself a defect — "nothing due" and "broken" look identical from `launchctl` — and telling them apart means reading
  `~/Library/Logs/CulebraLuxe/*.err.log`, which is a thing to ask for, never a reflex.
- **No Time Machine destination.** The repo, four branches and the exports live on one internal disk; iCloud is not a
  backup, and it is currently holding ~43 GB of dead targets (see next line). This is the biggest open risk.
- **The dead worktree records are gone (2026-10-03).** `git worktree list` shows only the four live trees
  (`Culebraluxe-web` and the three lanes): `/private/tmp/ocwt` is no longer registered (nor is its `refs/wip/ocwt`
  snapshot), and neither `~/Documents/Culebraluxe-web-roles` (33 GB, clean, HEAD `30b53b5d` — present in main) nor
  the orphan `~/Documents/Culebraluxe-web-claude` appears at all.
- **`build/rust` is warm (2026-10-03).** 4.5 GB in the shared `CARGO_TARGET_DIR`, so the cold compile has already
  been paid once and the next `cargo` invocation does not repeat it.

## The layout move (2026-10-01/02): what a lane does

**This describes a base that no longer exists, and the recipe that was used to leave it.** When it was written the three
lane worktrees sat on `97785410`, before the tree moved to the three tiers; that base is now an ancestor of `main`
(`git merge-base --is-ancestor 97785410 origin/main` → true, checked 2026-10-03), and each lane was caught up by
replaying the move, not by merging. The warning still holds for any lane on an old base, because the move landed once
and a lane that merges instead will be re-running it: do not merge `main` into a lane to catch up — the merge is a
conflict in every file that moved, and the tree that comes out of it is neither layout. Replay it instead — the script
is the move, written so it checks before it acts and running it twice does nothing.

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

Measured on 2026-10-03 in a rehearsal worktree at `97785410`: **51 rules apply, 0 stale**, the script exits 0, the tree
it produces holds 90 case files in `tests/tests/` with `rust/` and `deploy/` gone, and
`cargo check --workspace --all-targets` finishes with **0 errors**.

Two things the script deliberately does NOT do. It does not resolve the conflict between a lane's own work and the
layout — it moves the crate directories and re-spells the paths, and a lane's edits inside a moved file travel with it
because every step is a `git mv`. And it does not run `cargo fmt`: four module trees are deferred on purpose (a live
agent owns them), and a sweep that formats everything would collide with that agent's edits. Format the files you
touched, as always.

