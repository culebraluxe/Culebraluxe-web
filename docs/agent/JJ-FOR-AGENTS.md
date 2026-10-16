# `jj` for a git-trained agent

You know git. This repository's source-control interface is **Jujutsu** (`jj`). The state of that conversion is
`docs/agent/JJ-INTERFACE.md` — the card — and this file is the other half: the translation layer. It assumes your hands
know git and gives the command you should type instead, plus the four places this machine differs from a generic `jj`
tutorial.

Written 2026-10-15. Every `jj` claim below was run on this Mac against **`jj 0.46.0`**; every repository claim carries
the command that measured it. Where something is *not* converted yet, it says so instead of implying otherwise.

## Three facts that change every command later in this file

| fact | measured 2026-10-15 | what it means for you |
| --- | --- | --- |
| **Push with `jj push`**, not `jj git push` | `jj config list`; `aliases.push` = `util exec` → `bash -c bash "$(jj root)/scripts/jj-pre-push.sh"` | `jj push` runs the ported pre-push checks — the `Cargo.lock --locked` check and the `web/ui` wasm build — and then pushes. A bare `jj git push` skips them: the same choice `git push --no-verify` always was. |
| **`jj` cannot run in a lane yet** | `jj status` in `lane-claude` → `Error: There is no jj repo in "."`; `ls lane-claude/.jj` → absent | In a lane, git is still the interface — see *Where this file does not apply yet* at the bottom. This is the migration's next step, not a preference. |
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

**A teammate moved `main` and you have a git checkout (a lane, a relay worktree).** Until the lane converts, the git
form of the same three steps is:

```sh
git fetch origin main && git rebase origin/main
```

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

## Where this file does not apply yet

Three gaps, measured 2026-10-15. They are the conversion's remaining work, not advice:

1. **The lanes are git worktrees and have no `.jj`.** There are 23 worktree registrations on this machine
   (`git worktree list` — House Rule 4 owns the roster) and `jj status` fails in every `lane-*` with
   `Error: There is no jj repo in "."`. Until the lanes convert, a worker in a lane uses git for fetch/rebase/push
   (House Rule 2 names that fallback), and the conversion is one change: `scripts/lane-new.sh` → `jj workspace add`,
   with the ten worktree-aware scripts still seeing the lane.
2. **The pre-push checks are an alias, not a guard.** `jj` runs no git hooks, so nothing forces the `Cargo.lock` and
   wasm checks; `jj push` runs them because it is the alias this repository installed. A bare `jj git push` is the
   hole, and the only thing standing in it is your habit of typing `jj push`.
3. **The agent config does not know `jj` exists.** `jj` appears **zero** times in `opencode.json`, `forge/src`,
   `cli/src`, `agent-runtime` and `legacy/workflow_app/forge` (grep, 2026-10-15). So a Forge role with read-only
   authority — whose shell rules are a declared allow list plus a catch-all deny
   (`forge/src/engine/opencode_agents.rs:738`) — cannot run `jj status` at all, while `PUBLICATION_DENY`
   (`forge/src/engine/opencode_agents.rs:703`) names `git push`, `git remote` and `git fetch` and **no** `jj` verb.
   Both halves matter: the control that keeps publication Forge-owned does not cover the interface this handbook now
   tells an agent to publish with. Forge's own publication path is deliberately still git
   (`forge/src/engine/git_publish.rs:174` pushes `<sha>:refs/heads/main`) and should stay that way — it is not a
   model's shell.

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
