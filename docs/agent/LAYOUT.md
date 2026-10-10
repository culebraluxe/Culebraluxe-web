# Machine layout: where the code lives, and why

Written 2026-10-01, when the tree moved out of `~/Documents`. This is the machine half of the handbook —
`AGENTS.md` says how to work, this says where.

## The tree

```
/Users/Shared/dev/
├── src/
│   ├── Culebraluxe-web/     main checkout,   branch main        (the repo you are in)
│   ├── lane-forge-observations/ git worktree, branch lane/forge-observations
│   ├── lane-claude/         git worktree,    branch lane/claude
│   ├── lane-deep/           git worktree,    branch lane/deep
│   ├── lane-fledge/         git worktree,    branch lane/fledge
│   ├── lane-gpt/            git worktree,    branch lane/gpt
│   ├── lane-grok/           git worktree,    branch lane/grok
│   ├── lane-muse/           git worktree,    branch lane/muse
│   ├── lane-muse-2/         git worktree,    branch lane/muse-2
│   ├── lane-lightning/      git worktree,    branch lane/lightning
│   ├── lane-ling/           git worktree,    branch lane/ling
│   ├── lane-longcat/        git worktree,    branch lane/longcat
│   ├── lane-mimo/           git worktree,    branch lane/mimo
│   ├── lane-nemotron/       git worktree,    branch lane/nemotron
│   ├── lane-nemotron-2/     git worktree,    branch lane/nemotron-2
│   ├── lane-nemotron-lightning/  git worktree,  branch lane/nemotron-lightning
│   └── lane-spacebunny/     git worktree,    branch lane/spacebunny
└── build/
    ├── rust-main/           cargo target dir for the main checkout and the launchd jobs
    ├── rust-lane-<name>/    cargo target dir for one lane, one per lane (made on first build)
    ├── playbooks/           the Master copy of the Maestro playbooks (each lane carries its own copy)
    └── logs/                launchd job logs (wip-snapshot.log, wip-snapshot.err.log)
```

A Forge/Maestro sandbox — any checkout a command makes and removes — is the one kind of checkout with no directory
here: it builds into `<its own tree>/.cargo-target`, so its output dies with it. See "One target directory per
checkout".

Nothing else belongs in `src/`: no exports, no caches, no second copies of the repo.

**The lanes (13 worktrees on 2026-10-04, 15 on 2026-10-05).** The picture above is a picture: **`git worktree list` is the roster**, and a
lane exists only while it is in that output. One lane per agent, named for the model that works in it, because a second
directory for the same model is the first lane wearing a card. Two deliberate exceptions stand beside that rule —
`lane-muse-2` and `lane-nemotron-2`, second seats for two models, created on the Captain's call 2026-10-04 — and they are
seats, not identities: each is named for the same model as its sibling, holds no work of its own until a session is run in
it, and takes its place in `git worktree list` like any other lane.

**A third seat was added on the Captain's call 2026-10-05: `lane-nemotron-lightning`**, a lane for the Nemotron 3.5
Lightning model. `lane-lightning` — created 2026-10-03, listed in the table below as the Lightning lane, and holding no
session and no Maestro tab on 2026-10-05 — already carries that model, so the two are two seats of one fact rather than
two facts; retiring the older one is `git worktree remove ../lane-lightning && git branch -d lane/lightning`, and
re-founding it in place is `git worktree move` plus `git branch -m`.

| lane | model (ranked for this repo, 2026-10-03) | what it is for |
| --- | --- | --- |
| `lane-claude`, `lane-deep`, `lane-gpt`, `lane-grok`, `lane-muse` | the first five, one per agent | the standing agent set |
| `lane-nemotron` | Nemotron 3 Ultra — #1 | serious coding, architecture, debugging |
| `lane-mimo` | MiMo V2.6 Flash — #2 | general Smith work, bug fixing, cheap execution |
| `lane-spacebunny` | Space Bunny — #3 | experimental repo work, big context, second opinions |
| `lane-ling` | Ling 3.1 Flash — #4 | agent work, codebase analysis, well-scoped implementation |
| `lane-lightning` | Nemotron 3.5 Lightning — #5 | tests, repetitive implementation, subagent execution |
| `lane-longcat` | LongCat 2.5 Preview — #6 | large-context experiments; not a first choice for Rust |
| `lane-muse-2` | Muse — second seat | a second session of `lane-muse`'s model, run beside it (Captain, 2026-10-04) |
| `lane-nemotron-2` | Nemotron 3 Ultra — second seat | a second session of `lane-nemotron`'s model, run beside it (Captain, 2026-10-04) |
| `lane-nemotron-lightning` | Nemotron 3.5 Lightning — a seat beside `lane-lightning` | a session on the Lightning model, created through `pnpm lane:new` (Captain, 2026-10-05) |
| `lane-fledge` | Fledge | the model's seat, created through `pnpm lane:new` on the Captain's call (2026-10-05); rank not yet recorded |
| `lane-forge-observations` | Codex | temporary isolated lane for the five verified Forge repairs (2026-10-10) |

The six model lanes were created 2026-10-03 as **light lanes** (see "A lane is 84 MB" below): checkout plus env links,
no `node_modules` until a lane actually needs to build the website. `pnpm wip:now` covers them from their first minute,
because the snapshot job walks `git worktree list` rather than a list of names. The two second seats followed the same
recipe on 2026-10-04 through `pnpm lane:new` — 85 MB each by `du -sh`, no `node_modules`, upstream unset — so the snapshot
job covers them from their first minute too. `lane-nemotron-lightning` followed that recipe again on 2026-10-05 — 86 MB by
`du -sh`, 0 dirty files, no `node_modules`, no target of its own, upstream unset — so the snapshot job covers it from its
first minute as well. `lane-fledge` followed that recipe on 2026-10-05 — 87 MB by `du -sh`, 0 dirty files, no
`node_modules`, no target of its own, upstream unset — so the snapshot job covers it from its first minute too. The same day,
`lane-ling` and `lane-longcat` — the two of the three names the Captain asked for that already existed — were rebased from
`08717aea` onto `07d7f482`, closing a 103-commit gap without a merge.

## Lanes

```sh
pnpm lane:new <name>                  # scripts/lane-new.sh — the steps below, in this order
pnpm lane:new <name> --with-website   # ... and `pnpm install --frozen-lockfile` in that lane (30 MB)
pnpm lane:new <name> --dry-run        # print the commands, change nothing
```

What the script does, written out — for when it has to be done without it:

```sh
cd /Users/Shared/dev/src/Culebraluxe-web
git worktree add ../lane-<name> -b lane/<name> origin/main
cd ../lane-<name>
ln -sfn /Users/Shared/dev/.env.local     .env.local        # a symlink, never a copy
ln -sfn /Users/Shared/dev/.env.scheduler .env.scheduler
bash /Users/Shared/dev/src/Culebraluxe-web/scripts/lane-cargo-config.sh --at .   # this lane's own build dir
git branch --unset-upstream                                # a bare `git push` must not aim at main
chmod -R go-rwx .                                          # a fresh worktree lands 755 inside the 700 tree
# pnpm install --frozen-lockfile   # ONLY in a lane that must build the website or run tailwind
```

**A lane is 84 MB of code: the recipe is `git worktree add`, two symlinks, one line of cargo config and the modes, and
`pnpm install` is not part of it (2026-10-03; the cargo-config line arrived 2026-10-07).** That 84 MB has never included
build output and still does not: the lane's target directory lives outside the tree, at `build/rust-lane-<name>` — see
"One target directory per checkout" below. Measured that day: a plain lane is 84 MB on disk, and creating six of them moved `df` by
0.56 GB (813.21 → 813.77 GB used) — 93 MB each, of which 74 MB is `public/images`, which every checkout materialises
because it is tracked. A second lane's `pnpm install --frozen-lockfile` (3.9 s, rc=0) cost **30 MB** of disk while
adding **847 MB** of `node_modules`, because pnpm imports by APFS clone and the blocks stay shared: `du` counts that
847 MB once per lane and reports a 931 MB lane, which is how a lane came to look like it cost a gigabyte. **`du` counts
clones; `df` is the truth.** What a lane gives up without `node_modules` is the *website*, not the Rust:
`node_modules`-free `lane-nemotron` ran `cargo check -p workflow --all-targets` → rc=0 in 15.0 s and
`pnpm -s forge:sync-agents --check` → rc=0, and `.githooks/pre-push` runs `cargo` only, so a light lane can build,
test, commit and land. `pnpm build`, tailwind and `pnpm dev` are what need it.

**Taken further the same day, on the Captain's instruction: a lane is code and nothing else, and only the main checkout
carries the weight.** The seven lanes that had a `node_modules` had it removed (847 MB of `du`, 30 MB of `df`, each);
`pnpm lane:new <name>` is now the recipe as a script; and the cargo estate sits one directory per checkout, as "One
target directory per checkout" below describes — rewritten on the Captain's instruction 2026-10-07, when the shared
`build/rust` this sentence used to point at was retired. A lane that wants the website back runs `pnpm install` in that
lane, at 30 MB.


**One env, symlinked, never copied (2026-10-03).** `.env.local` (53 keys, including `DATABASE_URL_PROD` and the
production Mux credentials) and `.env.scheduler` (`FORGE_STORY_WORKERS`, `FORGE_PROVISION`, `FORGE_ALLOW_PUBLISH`) live
once, at `/Users/Shared/dev/.env.local` and `/Users/Shared/dev/.env.scheduler`, mode `600`, **outside every checkout**;
each checkout root holds a symlink to them. Before this, four byte-identical copies sat in four worktree roots — four
rotation points for one set of secrets, and a rotated key left stale copies alive in lanes nobody looked at. Both loaders
resolve the link: `scripts/dev.sh` reads `.env.local` from `$root` (`[ -f ]` and `done <` follow symlinks) and the Rust
CLI opens `repo_root()/.env.local` through `dotenvy::from_path` → `File::open`. `.gitignore:16` (`.env*`) and `:23`
(`.env.scheduler`) keep the link itself invisible to git, so every lane's `git status` stays clean, and because the real
file is outside every worktree it can no longer be committed from one. A new lane runs `ln -sfn`, never `cp`.

One lane, one branch, one agent: commit on the lane branch, land it on `origin/main`, and mind house rule 1
(`lane/*` is short-lived). **`origin/main` is the trunk, not the main checkout's local `main`:** Forge's publish path
(`forge/src/engine/git_publish.rs`) pushes candidates straight to `origin/main`, so a local `main` is always behind
something. Check out by rebasing (`git fetch origin && git rebase origin/main` in the lane), never by merging `main`
in; check in with `git push origin lane/<name>:main`, which is fast-forward-only — a lane that has not rebased is
refused instead of merged. Nobody commits on the main checkout; it follows the trunk with `git pull --ff-only`.
No lane reads another lane's tree — work crosses lanes only through `main` (`AGENTS.md`,
the 2026-10-03 lane exception to NO TREES). Lanes are worktrees, so they share history and one object store — that is the point.
**One target directory per checkout (2026-10-07).** The cargo target directory is per checkout, not shared:
`/Users/Shared/dev/build/rust-lane-<name>` for a lane, `/Users/Shared/dev/build/rust-main` for the main checkout and the
launchd jobs. It is named by a checkout-local `.cargo/config.toml` (written by `scripts/lane-cargo-config.sh`, which
`scripts/lane-new.sh` calls for every new lane), *not* by a shell export: the agents running on this Mac carry no
`CARGO_TARGET_DIR` at all, and an exported value outranks every config file — which is why the two exports that used to
name the shared directory (`~/.zshenv`, `~/.zshrc`) were removed rather than repointed. `~/.cargo/config.toml` keeps the
machine-wide facts: the fallback `rust-main` for a checkout with no config of its own, and `jobs = 4` — fifteen lanes can
now build at once, and fifteen times cargo's default of ten jobs would be 150 rustc processes on 10 cores and 32 GB,
trading an idle queue for a thrashing box.

**A disposable checkout builds inside its own tree (2026-10-08).** The one exception to that layout is the shape
`git worktree add` makes and `git worktree remove` takes away — a Forge/Maestro sandbox, or the detached checkout
`forge/src/engine/worktree.rs` uses to prove an integration commit — and it is deliberate: those are not lanes, and
their build output is `<their own tree>/.cargo-target`, so it is deleted with the checkout that owned it. Until
2026-10-08 they shared the `rust-main` fallback, which serialised a dozen sandbox worktrees against the main checkout
and the launchd job exactly the way the shared directory used to. The name is `.cargo-target` and *not* `target` on
purpose: `scripts/rust-ui-build.sh` reads cargo's answer `<checkout>/target` as "we are in the container" and builds
into `/target`, read-only on macOS — so an in-tree directory called `target` makes `cargo check` green while
`pnpm ui:build` and the deploy die, which is the trap the paragraph below records. A lane still builds into
`build/rust-lane-<name>`, outside its tree: a lane is not disposable and its output is something a human manages.
The naming rule lives in one place, `scripts/lane-cargo-config.sh`, and is applied by `.githooks/post-checkout`,
which git runs inside each new worktree — one hook covers every creator (lane-new.sh, the engine, Maestro, a human
typing `git worktree add`) instead of the same rule copied into four of them. The hook calls the script **from the
checkout it is creating**, so the copy that runs is the one at the commit being checked out: a change to the naming
rule takes effect for the next worktree only once it is committed, and whatever is wrong at `HEAD` is what every
`git worktree add` pays. That is worth remembering before blaming the hook for slowness — on 2026-10-08 an
**unquoted** heredoc in that script ran the backtick-quoted commands its own prose quotes (`git worktree add`,
`pnpm ui:build`, `cargo check`), so one sandbox took 30 s instead of 0.6 s and git's usage text appeared inside the
hook; the heredocs there are quoted delimiters now, and unquoting one to "let a variable expand" would restore the
bug. The one value that must expand is appended by `printf`.

The proof for all of it is `scripts/verify-checkout-build-dir.sh`, run from any checkout as
`bash scripts/verify-checkout-build-dir.sh`. Eighteen asserting checks, not a print-out: the hook fires in the new
worktree and that checkout is fast; the config it writes is `<tree>/.cargo-target` at mode 600; the directory is
git-ignored and leaves nothing dirty; cargo's own answer *standing inside the sandbox* is neither `/target` (the
`rust-ui-build.sh` trap) nor `build/rust-main`; two sandboxes get two different directories; a `lane-*` checkout is
treated as a lane; the main checkout is refused rather than given a second answer, and a checkout that already has one
keeps it; and a branch switch inside a sandbox leaves the config byte-identical and says nothing. It measures the
worktree it creates, so it is the regression guard for the 32 s bug above as much as the receipt for the rule.

**Why the shared directory went (2026-10-07), in two measurements.** One: it never held one copy of anything — cargo
keys an artifact by the feature/target combination it was built for, and that directory held 865 rlibs for 261 crates
(2026-10-03) — `libsyn` sixteen times, `libsqlx_postgres` **forty-five** times by 2026-10-07, `serde` eleven, `yew` nine
— because `-p <crate>`, `--all-targets`, `--features wasm` and the wasm target each key their own. So the sharing paid
for one directory and bought no deduplication at all. Two: cargo takes an *exclusive* lock on `<target>/.cargo-lock` for
the length of a build, so with fifteen lanes on one target the second `cargo` of the day did not run slowly, it did not
run: 24 of 24 samples over 24 s showed the lock held with no idle gap between the bots' compiles and two waiting cargos
with no children, while load sat at 10.2 on 10 cores. A queue in front of an idle CPU is the failure this layout
removes; it also removes the cross-lane staleness that a shared directory causes (below), and the 9.9 GB that directory
held comes back when it is deleted.

Measured that day, `build/rust` held **63 GB**: 34 GB `debug/deps`, 23 GB `debug/incremental`, 1.3 GB `release`, 463 MB
`wasm32-unknown-unknown`. The incremental directory is the part that regrows by itself — it went 16 GB → 23 GB inside one
session, from three `cargo check` runs, and nothing in the workspace asked for it (`Cargo.toml` declared no `[profile]`,
so it was cargo's default for `dev`). Three levers; the first two were taken on 2026-10-03, and the third is the standing
one:

1. `rm -rf build/rust/debug/incremental` — **done**: 23 GB back at once (`df` 803.9 → 787.6 GB used, `build/rust`
   52 → 36 GB); the seven lanes' `node_modules` came off in the same pass.
2. `[profile.dev] incremental = false` + `debug = "line-tables-only"` in the workspace `Cargo.toml` — **done**: the
   23 GB cannot grow back, and backtraces keep file and line. It re-fingerprints everything once, so it is a full
   rebuild; both lines carry their own comment, including how to raise `debug` again for a debugging session.
3. `cargo clean` in the checkout whose directory is large — the standing lever, and now a *smaller* one than it was:
   with one directory per checkout, `cargo clean` in a lane throws away that lane's artifacts and nothing else, where it
   used to remove every root's and every flag set's copies in one go (which is how the shared `build/rust` came down from
   36 GB to 9.9 GB, and why a `cargo clean` used to be a machine-wide event to be scheduled rather than a local tidy-up).

Retiring a lane has three lines now, not two: `git worktree remove ../lane-<name>`, `git branch -d lane/<name>`, and
`rm -rf /Users/Shared/dev/build/rust-lane-<name>` — the third is new on 2026-10-07 and it is the one that takes that
lane's build output back, immediately and without touching any other lane. (Before, a lane's artifacts stayed inside the
one shared directory until somebody ran one of the levers above.)

A shared target had a second failure mode, and it cost more than the lock: **a lane could be handed a stale artifact and
lose an hour to an impossible error (2026-10-03).** One directory per checkout closes that path — nothing a lane compiles
beside can have come from another lane's build — but a stale artifact *inside* your own directory is still possible, so
the rule this pair of paragraphs ends with still stands. `cargo check --workspace --all-targets` in `lane-deep` reported
`E0599` twice — `reset_forge_attempts` and `story_repair_counts` "not found" on `ForgeEngineDao`, both defined in
`db/src/forge_engine.rs` at HEAD (`db_ledger.rs:98`, `db_writer.rs:17`) — reproducibly, while `cargo check -p forge
--all-targets` passed and the same commit was green in the main checkout against the same target dir. `cargo clean -p db
-p forge`, and the identical command went green; the mechanism is a `db` artifact from another lane's build being reused
under an mtime-based fingerprint, which the clean removed. **So: when a compile error names a method that is in the file
you are reading, run `cargo clean -p <crate>` before believing it, and never report a red T0 you have not reproduced
after a clean.** A phantom red is as expensive as a phantom green.

**The same trap has a second shape, hit on 2026-10-04: a missing FIELD, not a method.** `cargo test -p cli` in `lane-deep`
died on `E0063: missing field \`signed_media_id\` in initializer of \`LuxesignFinalizeResult\`` at
`web/src/luxesign/mod.rs:1283` and `:1350` — while `middle/model/src/luxesign.rs` defines that struct with three
fields and the string `signed_media_id` exists in no model source in either checkout. What forced the mismatch was a
`web → forge` path dependency that had just changed (the adapter commit), so `web` recompiled while the `model` artifact
beside it stayed "fresh" by mtime from another lane's build: the recompile answered about *that* lane's struct. `touch
middle/model/src/luxesign.rs web/src/luxesign/mod.rs` — no `cargo clean` needed — and the identical command
went green in 13.6s. So the rule generalizes to any symbol: **when a compile error names a method, field, type or variant
that the file you are reading does not have, you are compiling a stale artifact; bump the mtime of the files that define
it, fall back to `cargo clean -p <crate>`, and never report the red.** (Until 2026-10-07 that stale artifact could be
another lane's, because one target directory served them all; now it can only be your own — a build interrupted
mid-flight, or a checkout that changed under it — which is why the rule stands but the cross-lane blame does not.) That afternoon already read as "trunk is
broken, and the other agent's commit did it" — worth a lane's window and a wrong accusation — one paragraph after the
sentence that bans exactly that report, written the day before.

## Rules that keep this working

1. **Never work in `/tmp` or `~/Documents`.** macOS purges `/tmp`, and iCloud resurrects deletions in `~/Documents`;
   both leave dead `git worktree` records behind, and a dead record makes `git worktree list` lie about what work exists.
2. **No stores inside a checkout.** pnpm's store is `~/Library/pnpm/store/v10`; a `.pnpm-store` inside the repo is a
   leftover from an older config (one was removed on 2026-10-01, 936 MB, referenced by no `.npmrc`).
3. **`700`, on purpose.** A second account (`cecochran`) exists on this Mac and `/Users/Shared` is world-readable, so
   the tree, the lanes and every `.env*` file are owner-only. Do not loosen the modes. Unix does not inherit modes —
   a new lane or build directory is created as `mode & ~umask`, which is how a fresh `git worktree add` under a default
   `022` shell lands at `755` inside the `700` tree: `chmod -R go-rwx /Users/Shared/dev` puts it back (never `-R 700`
   or `-R 600` — the first adds an execute bit to every file, the second strips it from every script and binary in
   `node_modules` and `build/`), and `umask 077` stops it recurring. The `700` on `dev/` is what actually gates
   access — nothing inside is reachable by the other account while it holds — so this is defence in depth, not the door.
4. **Never run a command that pages or waits for an editor.** `core.pager` is `cat` machine-wide for a reason — see below.
5. **Uncommitted work is snapshotted, not lost.** `scripts/wip-snapshot.sh` writes every dirty worktree to
   `refs/wip/<name>` every 5 minutes (launchd, `pnpm wip:install` / `wip:now` / `wip:uninstall`):

   ```sh
   git --no-pager diff HEAD refs/wip/lane-gpt --stat    # what the snapshot holds
   git show refs/wip/lane-gpt:path/to/file              # a specific file
   ```
6. **A lane holds code and nothing else.** No `node_modules`, no `target` inside the tree, and no `pnpm install` "to be
   safe": a lane's cargo output goes to `build/rust-lane-<name>`, outside the checkout (see "One target directory per
   checkout" above — a *sandbox* is not a lane and has no directory there at all: it builds into its own tree, because
   it dies with it), and `pnpm install` is the per-lane call of a lane that must build the website (30 MB,
   `--with-website`). `pnpm lane:new <name>` is the whole recipe — it writes the lane's own cargo config too — and it
   leaves the upstream unset so a bare `git push` in a lane cannot aim at `main`.

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
  The jobs keep their logs in `/Users/Shared/dev/build/logs` — the shared machine log tree, outside every checkout —
  and asking to read one is still a thing to ask for, never a reflex.
  **Verified 2026-10-03 (lane-deep): the repoint is real but not sufficient — the job is dead.** `plutil -p` shows
  `AGENT_WORKER_REPO=/Users/Shared/dev/src/Culebraluxe-web` and `StartInterval 180` as intended, but the *deployed
  wrapper* (`~/Library/Application Support/CulebraLuxe/agent-worker-once.sh`, line 229) still runs
  `--manifest-path rust/Cargo.toml`, a path the 2026-10-02 move deleted — so every 180-second tick fails before the
  Rust worker starts, which is why `launchctl list` shows `agent-worker` with PID `-` and exit 1. The plists were
  repointed; the bodies they invoke were not. Fix: `pnpm agent:scheduler:install` from the main checkout (it
  rewrites the wrapper *and* the plist's `AGENT_WORKER_LOG_DIR`); the Apple jobs need their own `install` for the
  same reason.
- **No Time Machine destination.** The repo, its lane branches and the exports live on one internal disk; iCloud is not a
  backup, and it is currently holding ~43 GB of dead targets (see next line). This is the biggest open risk.
- **The dead worktree records are gone (2026-10-03).** `git worktree list` shows only live trees — `Culebraluxe-web`
  plus the lanes, no estate: `/private/tmp/ocwt` is no longer registered (nor is its `refs/wip/ocwt`
  snapshot), and neither `~/Documents/Culebraluxe-web-roles` (33 GB, clean, HEAD `30b53b5d` — present in main) nor
  the orphan `~/Documents/Culebraluxe-web-claude` appears at all.
- **The target dir was 63 GB, then held down, then split per checkout (2026-10-03 → 2026-10-07).** The cold compile has
  been paid; what made it 63 GB was 23 GB of `incremental` scratch plus a copy of every crate per flag set (865 rlibs for
  261 crates). The first cause is closed in the repo (`incremental = false`, `debug = "line-tables-only"` in `Cargo.toml`);
  the second was never a thing sharing could fix, so on 2026-10-07 the one directory became one per checkout — a lane
  keeps `build/rust-lane-<name>` (made on its first build, removed with the lane) and the machine-wide fallback is
  `build/rust-main`. The numbers, with the `df` readings, are in "One target directory per checkout" above; the `4.5 GB`
  this line claimed until 2026-10-03 was a guess, and a wrong one.
- **Maestro writes into the lanes (found and closed 2026-10-03).** The coworking MCP app (`Maestro.app`, PID 1181) was
  running with its playbooks under version control: `lane-claude` held six new files in `.maestro/playbooks/Initiation/`,
  and `lane-muse` four renames plus a new `.maestro/playbooks/message-bus/`. The Captain closed it that day and is
  testing models with it off, so those fifteen files are left exactly as they are — neither committed nor reverted, in
  case the exercise resumes. The forge-produced `TST-WF-DECISION-004/005/006` tests on `main` came from the same
  exercise, and their rustfmt hunks are what kept `gates` red from 01:10 until `8bc0aaf3` formatted them.

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
