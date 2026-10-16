# `jj` for a git-trained agent

You know git. This repository's source-control interface is **Jujutsu** (`jj`). The state of that conversion is
`docs/agent/JJ-INTERFACE.md` — the card — and this file is the other half: the translation layer. It assumes your hands
know git and gives the command you should type instead, plus the four places this machine differs from a generic `jj`
tutorial.

Written 2026-10-15; the lanes were converted to `jj` workspaces on 2026-10-16 and every row that said otherwise was
corrected that day — fact 2 and *Where this file does not apply yet* keep the old reading beside the new one, because a
docs change that quietly drops the sentence it falsified is how the next reader re-measures what is already known. Every
`jj` claim below was run on this Mac against **`jj 0.46.0`**; every repository claim carries the command that measured
it. Where something is *not* converted yet, it says so instead of implying otherwise.

## Three facts that change every command later in this file

| fact | measured 2026-10-15, re-checked 2026-10-16 | what it means for you |
| --- | --- | --- |
| **Push with `jj push`**, not `jj git push` | `jj config list`; `aliases.push` = `util exec` → `bash -c bash "$(jj root)/scripts/jj-pre-push.sh"` | `jj push` runs the ported pre-push checks — the `Cargo.lock --locked` check and the `web/ui` wasm build — and then pushes. A bare `jj git push` skips them: the same choice `git push --no-verify` always was. |
| **`jj` runs in every lane now** (converted 2026-10-16 — this row was *falsified* that day) | `jj workspace list` → **24** workspaces = `default` + the 22 `lane-*` + the Forge sandbox `worktrees/wo/204`; `git worktree list` → the same 24; `jj -R ../lane-bot3 status` → `The working copy has no changes.` The old reading, replaced: *"`jj` cannot run in a lane yet"*, measured 2026-10-15 as `jj status` in `lane-claude` → `Error: There is no jj repo in "."` | Type `jj` in a lane exactly as in the main checkout — inspect, describe, rebase, fetch, push. Nothing in this file's git fallback applies to a lane any more (it survives only for a disposable Forge/Maestro checkout). `jj push` is installed repo-wide, so a lane gets the checks too: `jj -R ../lane-claude config get aliases.push` prints the alias. |
| **`main` is protected** | `gh api repos/culebraluxe/Culebraluxe-web/branches/main/protection` → `{"force_push": false, "admins": true}` | You cannot rewrite published history, admin or not. You land **on** `main`; you never rewrite it. Rewriting it needs the Captain (card, item 4). |

`jj` is not a second source-control system. It keeps its objects in a git object store and `jj git push` speaks the git
protocol to GitHub, so `git` stays installed and `origin` stays a git remote. What changes is what **you** type, and
with it the habits that lose work: waiting for a commit to be "ready", hiding work in a stash, parking a branch.

## The mental model, as a translation

| git | `jj` |
| --- | --- |
| working directory → staging area → commit → branch | the **working copy is a commit** (`@`); every command snapshots it |
| `git add` decides what is in the commit | no staging area exists; there is nothing to forget |
| the commit id is the work | **change id** (survives rewrite) + **commit id** (one snapshot) — think in changes |
| a branch is where you are | a **bookmark** is a named pointer; you are always *on a change* |
| `HEAD`, detached `HEAD`, dirty tree, stash | none of these states; `jj op log` + `jj undo` instead |
| reflog (last resort, opaque) | **operation log** — every ref movement, readable and reversible |

Two consequences worth internalising before the commands:

- **Nothing is "uncommitted".** If you edited a file, it is already in `@`. There is no state to lose while you think.
- **Rewriting your own history is cheap and normal.** Editing an old change, squashing, splitting and rebasing are
  routine. That is private history; `main` above it is the opposite (fact 3).

## `@` and the two identities

```
jj log            # @ is your working-copy change; @- is its parent   (illustration, not your log)

@  kxryzvtt 8ac04b77  Add entitlement service     ← change id, commit id, description
○  tznzmylu 2bb77881  main | docs(agent): …
```

`@` is where your edits belong. `@-` is its parent. The **change id** (`kxryzvtt`) is the logical piece of work and
survives a rebase, a squash or an edit; the **commit id** (`8ac04b77`) names one snapshot and moves when the change is
rewritten. Address changes by change id when you can — that is the identity that does not move under you.

## The five commands that cover most work

```sh
jj status                      # what is modified, conflicted, where @ and bookmarks are
jj diff                        # what is in @ right now
jj log                         # the graph
jj describe -m "what I changed"     # ALWAYS -m: a bare jj describe opens an editor and hangs (below)
jj new                         # start the next change, as a child of @
```

Verified to exist in `jj 0.46.0` on this machine, together with the rest of this file's commands: `jj status`, `jj diff`,
`jj log`, `jj show`, `jj describe`, `jj new`, `jj edit`, `jj split`, `jj squash`, `jj abandon`, `jj restore`, `jj undo`,
`jj duplicate`, `jj op log`, `jj op show`, `jj op restore`, `jj rebase`, `jj bookmark list`, `jj bookmark create`,
`jj bookmark set`, `jj bookmark delete`, `jj git fetch`, `jj git push`.

**`jj checkout` does not exist in 0.46** (`error: unrecognized subcommand 'checkout'`). Where a git habit says
`git checkout <commit>` to look at an old revision, use `jj show <rev>` to read it and `jj edit <rev>` to work on it.

## Everyday operations

**Describe the work, then start the next change.** There is no commit step; `describe` names the change you are already
in, and `new` opens a fresh child. This is how a clean stack is built — do not build one giant change:

```sh
jj describe -m "Add entitlement evaluation service"
jj new
```

**Fix a mistake in an earlier change.** `jj edit <rev>` puts `@` on it; edit normally; `jj new` when you are done. `jj`
rebases the descendants for you, which is most of what git history surgery was for.

**Split a change that holds two unrelated pieces of work.** The non-interactive form puts the named files in the first
commit and leaves the rest in a new child:

```sh
jj split web/src/entitlement.rs        # filesets given ⇒ no diff editor opens
```

`jj split --help` is explicit: `-i, --interactive` "is the default **if no filesets are provided**". An agent that runs
a bare `jj split` gets a diff editor — a hung window, not a split.

**Squash** only when the work genuinely belongs in the parent (`jj squash`), **abandon** a change you no longer want
(`jj abandon <rev>`, `jj abandon @` for the current one), **restore** files from the parent into the current change
(`jj restore web/ui/src/view.rs`) — that is `git checkout -- file` / `git restore file`'s replacement, and it is how you
throw away an edit you did not mean to make.

**Look before you rewrite.** `jj log` and `jj status` first; they are cheap and they are the difference between a
deliberate rebase and a surprise.

## Non-interactive, or it hangs

House Rule 9 exists because eight commands were found stuck in `less` for up to 26 hours on an unattended machine. `jj`
has its own pager and its own editor, so the rule applies twice over here.

| trap | measured | do this instead |
| --- | --- | --- |
| the pager is `less -FRXK` | `jj config get ui.pager` → `["less", "-FRXK"]` | `jj --no-pager log` (or `--no-pager` on any command), or set `ui.pager` to `cat` for a script's environment |
| `jj describe` with no `-m` opens an editor | `jj describe --help`: "Starts an editor to let you edit the description" | always `jj describe -m "…"` |
| bare `jj split` opens a diff editor | `jj split --help`: `-i` is the default with no filesets | `jj split <paths>` |
| `jj resolve` invokes an external merge tool per file | `jj resolve --help` | edit the conflict markers in the file directly, then `jj status` — see *Conflicts* below |
| `jj edit` with no revision, `jj squash -i`, interactive commands generally | — | never run an interactive command from an agent shell; every one of them waits for a keypress you cannot send |

## Bookmarks: pointers, not a place you are

```sh
jj bookmark list                        # what exists, and what it tracks
jj bookmark set my-work -r @            # create or move one
jj bookmark create my-work -r @         # create only; refuses if it exists
jj bookmark delete my-work
```

Bookmarks are the closest thing to git branches, and thinking of them as *where you are* is the habit to drop: you are
always on a change. Bookmarks matter at exactly two moments — naming a lane's work, and publishing.

## Fetching, syncing, and landing on `main`

```sh
jj git fetch                      # see what moved; never git pull
jj log                            # where is main@origin now?
jj rebase -d main@origin          # your work replays on top of it — a rebase, never a merge
jj bookmark set main -r @         # only when you are the one landing
jj push --bookmark main           # = scripts/jj-pre-push.sh (the checks) + jj git push
```

`main@origin` is what a teammate pushed. `jj rebase -d main@origin` is the whole of "sync" — there is no merge step and
no stash pop, because your work is already a commit. If you are the one landing a change onto `main`, the last two lines
are the landing: `jj bookmark set main -r <rev>`, then `jj push`.

The repository config also installs a revset alias for the same thing (`revset-aliases."trunk()" = "main@origin"`), so
`jj rebase -d trunk()` and `jj log -r 'trunk()..@'` work everywhere. Both spellings are the same revision.

**A teammate moved `main` and you have a git-only checkout.** Until 2026-10-16 this paragraph named a lane first; the
lanes are `jj` workspaces now (`jj workspace list` → 24), so the only checkouts left without `jj` are the fresh disposable
ones — a Forge/Maestro sandbox, or the disposable worker workspace `forge/src/engine/worktree.rs` makes with `git
worktree add -b <branch>` (`:298-308`; a worker workspace is removed by `cleanup_worker_workspace`, `:321`, and a lane's
checkout by `remove_lane_worktree`, `:180`) — see `docs/agent/LAYOUT.md`, "A disposable checkout builds inside
its own tree". The sandbox in today's roster (`worktrees-wo-204`) was adopted along with the lanes, so `jj` answers there
too; a *fresh* one has no `.jj`. In one of those, and only there, the git form of the same three steps is:

```sh
git fetch origin main && git rebase origin/main
```

**In a lane that is now the wrong reflex, and it is worth naming because the habit is older than the tool.** If you are
in `lane-<name>` and about to type `git fetch` or `git rebase`, `jj status` will answer there — use the `jj` lines above.
Two readings that used to identify a lane's branch no longer do: measured 2026-10-16 in `lane-claude` and `lane-bot3`,
`git branch --show-current` → `""` and `git rev-parse --abbrev-ref HEAD` → `HEAD`. The mechanism, measured across
checkouts that day: `jj` keeps the working copy in a commit of its own (`@`) and leaves git's `HEAD` detached at that
commit's **parent** (`Culebraluxe-web` `HEAD=2e05599e0` with `@=785786512a1b`; `lane-bot3` `32a92776d`/`8735056ef3ec`;
`lane-mimo` `d240acf14`/`40125edf4d8d`). So a lane's uncommitted work, which lives in `@`, reaches git as *staged* changes
against `HEAD` rather than as unstaged dirt — and a clean lane looks clean to git for the same reason.
**What identifies a lane is its workspace name** — `jj workspace root` answers
`/Users/Shared/dev/src/lane-bot3`, and `jj workspace list` shows every one — **not a bookmark**: run in a lane,
`jj bookmark list` prints every bookmark in the repository (20 of them are named `lane/*`, and `lane-claude` shares its
own target with 17 siblings), so grepping it for `lane` is how you find a wrong answer. Where a lane's work sits is
`jj --no-pager log -r @- --no-graph -T 'bookmarks'` (`@` itself carries none: it is the empty working-copy commit).

`jj git push` is force-with-lease by default — the push help says the remote is updated "only if its current state
matches what Jujutsu last fetched" — so a race **refuses** and names the cause instead of clobbering. A refusal is a
report, not a retry (House Rule 9): fetch, rebase, push again with the reason understood.

`jj git push --bookmark <name>` also tracks a bookmark the first time you push it ("If a bookmark isn't tracking
anything yet, the remote bookmark will be tracked automatically"), so a new `lane/<name>` needs no extra flag.
`--branch` still parses in 0.46 as the older spelling; `--bookmark` is the documented one — use it.

## Conflicts are a state, not an interruption

Git stops an operation, forces you to resolve, and makes you `--continue`. `jj` does not: a conflict lives **inside a
change**, the graph keeps existing, and you fix it by editing the file. So the protocol is a report, not a ritual:

```sh
jj status           # names the conflicted files
# edit the file: resolve the conflict markers, exactly as in git
jj diff             # look at what you resolved
jj status           # no conflicts left?
```

There is no `jj rebase --continue`. Never report a change that still shows a conflict as finished — House Rule 8's
receipt and the card's "never hide conflicts" mean the same thing here.

## If you break something

Do not start throwing commands at the repository. `jj` records operations, and that is what the log is for:

```sh
jj op log               # every operation: fetches, rebases, squashes, bookmark moves, abandons
jj op show <op-id>      # what one of them actually did
jj undo                 # walk the last operation back
jj op restore <op-id>   # put the repository back at an earlier operation
```

This is the difference between a bad rebase and a lost afternoon: `jj undo` is usually the whole recovery. The operation
log is durable, so it survives the session that caused the damage.

## Before you say the work is done

House Rule 8 (a receipt, not a claim) needs repository-specific commands, because "run the tests" is not an instruction:

```sh
jj status                                         # no conflicts, nothing stray
jj diff                                           # intended files only
jj log -r '::@'                                   # the change is where you think it is
cargo check -p <crate you touched>                # T0: it compiles
pnpm slice:check                                  # T1: the sections you touched (rustfmt + their crates)
pnpm protected-files                              # the protected-file gate
pnpm forge:guard-lint && pnpm forge:packet-lint   # the harness gates
```

Two things that bite on this repository in particular:

- **`Cargo.lock` travels with `Cargo.toml`** (House Rule 5). It is the first thing the pre-push checks will refuse.
- **Doc-only changes still owe the harness.** The change that added this file ran `pnpm protected-files`,
  `pnpm forge:guard-lint`, `pnpm forge:packet-lint` and `pnpm forge:sync-agents --check` before it landed. A docs change
  is not a free pass.

## Git → `jj`, the honest table

| git instinct | what to type |
| --- | --- |
| `git status` | `jj status` |
| `git diff` | `jj diff` |
| `git log` | `jj log` |
| `git show <rev>` | `jj show <rev>` |
| `git add` | nothing — there is no staging area |
| `git commit -m "…"` | `jj describe -m "…"` then `jj new` |
| amend | edit the current change; it is never "already committed" |
| `git checkout <commit>` | `jj edit <rev>` to work on it, `jj show <rev>` to read it (there is no `jj checkout`) |
| `git checkout -- file` / `git restore file` | `jj restore file` |
| `git branch` | `jj bookmark list` |
| `git switch -c foo` | `jj new` (and a bookmark only if something outside needs the name) |
| `git fetch` / `git pull` | `jj git fetch`, then `jj rebase -d main@origin` |
| `git rebase` / `git merge` | `jj rebase` — no merges are part of this workflow |
| `git push` | `jj push` (checks + push) — not a bare `jj git push`, which skips the checks |
| `git push --force` | never; `jj git push` is force-with-lease and `main` is protected |
| `git reset` | `jj abandon`, `jj restore`, or `jj undo` — pick by intent |
| `git stash` | **never** (see below) |
| `git cherry-pick` | `jj duplicate` / `jj rebase` |
| reflog recovery | `jj op log`, `jj undo`, `jj op restore` |

**`git stash` is forbidden here, and there is nothing to replace it with.** A stash is a commit nobody can find; this
repository was still carrying 16 of them on 2026-10-15, salvaged to `salvage/*` precisely because they were invisible
work. The safety net a stash pretends to be already exists: `scripts/wip-snapshot.sh` writes local `refs/wip/<name>`
commits every 5 minutes for every checkout (`pnpm wip:now`), never pushes and never takes a lock — and under `jj` your
edits are in a change anyway, so there is nothing to park.

## Where this file did not apply yet — four items, two now closed (measured 2026-10-15, re-checked 2026-10-16)

Four items, measured 2026-10-15 and re-read 2026-10-16: **item 1 closed on 2026-10-16** (the lane conversion below),
item 3 the day it was measured, and what remains is item 2 and the prose items 3 and 4 name. Work, not advice:

1. **CLOSED 2026-10-16 — every worktree registration on this machine is a `jj` workspace.** The conversion was run by
   `scripts/lane-adopt-jj.sh` (written for it: `--all` for the fleet, one lane at a time, `--dry-run`, and a kept-copy
   path for a re-run), and the receipts are: `jj workspace list` → 24 (`default` + the 22 `lane-*` + the Forge sandbox
   `worktrees/wo/204`), `git worktree list` → the same 24, `jj status` answers in each, every `refs/heads/*` ref is where
   it was (`lane-claude-deploy`'s detached HEAD still at `d240acf14`), and the kept copy written for each lane was proved
   content-identical to what the workspace loaded (`rsync -rcn` over every comparable entry — only mtimes differ). The
   old reading, replaced: *"The lanes are git worktrees and have no `.jj`"*, 23 registrations, `jj status` failing in
   every `lane-*` with `Error: There is no jj repo in "."`. The worktree-aware scripts still see every lane, because
   `jj workspace add` inside this colocated repo registers a git worktree as well — `unblock.sh` and `wip-snapshot.sh`
   were re-read on the day and see the lanes as `jj` workspaces, and `git worktree list` holds the same 24 entries it
   did before the conversion, which is what the other eight read.
2. **The pre-push checks are an alias, not a guard.** `jj` runs no git hooks, so nothing forces the `Cargo.lock` and
   wasm checks; `jj push` runs them because it is the alias this repository installed. A bare `jj git push` is the
   hole, and the only thing standing in it is your habit of typing `jj push`.
3. **The agent config knows `jj` now** (fixed 2026-10-15, the same day it was measured as *zero* occurrences across
   `opencode.json`, `forge/src`, `cli/src`, `agent-runtime` and `legacy/workflow_app/forge`). Both halves landed in the
   one renderer, `forge/src/engine/opencode_agents.rs`: a read-only Forge role's shell list (`INSPECT_SHELL`, and the
   test analyst's beside it) now grants `jj status`, `jj diff`, `jj log`, `jj show` and `jj bookmark list` — the commands
   J3, J4 and the core loop on the first page actually require — and `PUBLICATION_DENY` now denies `jj push`,
   `jj git push`, `jj git fetch`, `jj git remote`, `jj git clone` and `jj util` beside git's spelling of each verb, so
   publishing with `jj` is no longer the one spelling nobody denied. `jj util` is deliberate: `jj util exec -- …` is how
   the `jj push` alias itself is defined (in the repo config file `jj config path --repo` prints, not `.jj/repo/config.toml`;
   measured 2026-10-16). The repo-root `opencode.json` is generated from that
   renderer and drift-checked inside `pnpm forge:harness`, and both halves are asserted in
   `tests/tests/opencode_v2_agents.rs`. **The coupling to item 1 was discharged on 2026-10-16, and the prose it was
   holding back moved the same day:** the instruction an implementer reads said *"Create a local git commit with the
   intended changes when the story requires it"* (`forge/src/engine/packet.rs:338`) — TRUE in a lane while a lane had no
   `.jj`, false in all 24 workspaces — and it now states the verb the isolation line above it implies: `jj describe -m`
   then `jj new` in a `.jj` workspace, `git commit` in an isolated Git worktree, and never a bare `jj describe` (it opens
   an editor and hangs, J15). The read-only roles' git entries (`git status`, `git diff`, `git log` beside the `jj` ones
   at `forge/src/engine/opencode_agents.rs`) stay, for the reason the packet sentence now states out loud: a fresh
   Forge/Maestro sandbox is made by git and has no `.jj`, so those are the spellings that answer there. Forge's own
   publication path stays git on purpose (`forge/src/engine/git_publish.rs` pushes `<sha>:refs/heads/main`): it is not a
   model's shell, and that difference is what keeps the deny list meaningful.
   Forge's own
   publication path stays git on purpose (`forge/src/engine/git_publish.rs` pushes `<sha>:refs/heads/main`): it is not a
   model's shell, and that difference is what keeps the deny list meaningful.

4. **The conversion unblocked two pieces of prose that still describe the pre-`jj` lane, and neither is changed here.**
   (a) **Six control-plane scripts read a lane's branch the git way, and a lane answers empty now.** Measured in
   `lane-claude` and `lane-bot3` on 2026-10-16: `git branch --show-current` → `""`, `git rev-parse --abbrev-ref HEAD` →
   `HEAD`. `scripts/ops/recover/unblock.sh` and `scripts/agent-worker-once.sh` already handle exactly this and say so in
   a comment; these six do not — `scripts/ops/gate/slice-check.sh` (its receipt line prints `detached`),
   `scripts/forge-v10-release-receipt.sh`, `scripts/release-record.sh` (two sites), `scripts/vercel-deploy-prod.sh`,
   `scripts/vercel-release-prod.sh`, and `scripts/vercel-provision-rust-project.sh`, which is fail-closed: it refuses
   anything that is not `main`, so it is a refusal rather than a wrong receipt. **This bug class predates today** — the
   main checkout has been colocated since it moved to `jj`, so all six already answered that way there; what the
   conversion did is spread it to 22 more checkouts. The `jj` spellings are `jj workspace root` (or the `NAME: path`
   column of `jj workspace list`) for *which checkout is this*, and
   `jj --no-pager log -r @- --no-graph -T 'bookmarks'` for *where does its work sit* — not `jj bookmark list`, which
   prints every bookmark in the repository from any workspace. No script should read a lane's identity out of git any more.
   (b) **The two items in item 3 above** — the packet sentence and the read-only git grants — for the reason given
   there: the excuse that kept them is gone, and the change itself is not a docs change.

## The source-control rules in this file

Numbered **J1–J15, deliberately not the House Rules' numbers.** Where a cheat sheet would say "rule 10", this file says
J10, because `AGENTS.md` rule 10 *is* the `jj` interface rule, and a second meaning for one number is exactly how the
ambiguity that created this file starts.

| # | rule |
| --- | --- |
| J1 | Use `jj` as the source-control interface; git stays a read-only instrument (`git log`, `git ls-remote`, `git cat-file`, `git for-each-ref`). |
| J2 | No git staging. There is no `git add` step. |
| J3 | Inspect `jj status` before you begin. |
| J4 | Inspect `jj log` before you modify history. |
| J5 | Keep logical work in logical changes; `jj new` rather than stuffing unrelated work into `@`. |
| J6 | Describe every meaningful change (`jj describe -m`). |
| J7 | Use `jj split <paths>` when unrelated work got mixed; never a bare `jj split`. |
| J8 | Use `jj squash` only when the changes genuinely belong together. |
| J9 | Never overwrite or casually rewrite another agent's work. |
| J10 | Sync with `jj git fetch` + `jj rebase -d main@origin`; never `git pull`, never `git merge`. |
| J11 | Publish with `jj push` (it runs the checks); a bare `jj git push` is a deliberate skip, and `git push --force` is never allowed. |
| J12 | Bookmarks are publication pointers, not the unit of work. |
| J13 | Never report conflicted work as complete. |
| J14 | Run `jj op log` and consider `jj undo` before any complicated manual recovery. |
| J15 | Never run an interactive command from an agent shell: `jj describe` without `-m`, a bare `jj split`, `jj resolve`, `jj squash -i`, or anything whose output can page through `less`. |

Read `docs/agent/JJ-INTERFACE.md` before your first command: it is the file that says which of these rules the
repository can actually enforce today and which are still on your honour.
