# The interface is `jj` — one card for the version-control layer

Written 2026-10-15 on the Captain's direction, given 2026-10-14 in his words: *"i want them to stop using git
and use just JJ"*. Until this file existed that direction lived only in a chat window. Measured the same day:
`AGENTS.md`, `CLAUDE.md`, `docs/agent/**` and all 17 `.opencode/skills/` cards mentioned `jujutsu` **zero**
times, and `git log --all -S'jujutsu'` found **no commit in any ref** that ever added the string. Ten days of
workers obeyed a handbook whose rule 2 told them to type `git pull --rebase && git push`, which is exactly what
it said. That is this repo's recurring failure in one line: **an instruction that was given and never written
down is not a rule, it is a conversation.**

## Why — the measured cost, not the argument

The Captain's reason: *"git has been 4 days of problems … this is my last attempt to make it work before i rip
it out"*, and *"we spent last 2 days recovering lost code that was done … it is mostly tests but still i paid
for work to be done."* On 2026-10-15 that cost was measured instead of described:

| what | number | how it was measured |
|---|---|---|
| lanes parked on one 3-day-old commit | **18 of 22** at `d240acf14` (2026-10-12) | `git worktree list` |
| stranded `agent/*` branches with landed-nothing | **at least 13**, carrying **at least 26** commits | `git rev-list --count origin/main..<branch>` |
| a live lane's work nobody could pull | **3 commits + 5 dirty files** in `lane-nemotron` (`wo/301`), including `web/ui/src/app/file_registry.rs` and a new test | `git -C lane-nemotron log origin/main..HEAD`, `status --porcelain` |
| work parked in a place nobody reads | **16 stashes**, some naming the near-losses ("re-stashed after accidental pop", "stale forge claim/qa drafts … before rebase") | `git stash list` |
| backups invented to survive the above | **at least 12** `refs/wip/*` refs | `git for-each-ref refs/wip/` |
| tools written to find work again | **3**: `recover:strand`, `recover:unblock`, `recover:relay` | `package.json` |

None of that is a permissions problem. git never refused the work; **the shape of the workflow** (long-lived
branches, per-lane trees, stashes, and nothing that forces work to become visible) is what let paid work sit
outside `main` for days. The pre-push hook's own comment says it better than this card can: *"refusing them did
not stop branching, it only made the branch invisible — five days and 19 commits, found by `pnpm recover:strand`
on 2026-10-02."*

## What `jj` is, and what it is not

It is **not** a different source-control system to replace git with. `jj` keeps its objects in a
git-compatible object store and `jj git push` speaks the git protocol to GitHub — you cannot uninstall git and
keep `origin`. What the switch removes is agents *typing* git, and the branch/stash/tree habits that lose work:

- **the working copy is always a commit.** `jj` snapshots it on nearly every command, so "leaves code
  unchecked in" stops being possible: there is no staging area to forget and no dirty tree to abandon. House
  Rule 7 ("leave the kitchen clean") becomes structural instead of a promise.
- **every operation is recorded.** `jj op log` is a durable, timestamped history of every ref movement in the
  repo, and `jj undo` / `jj op restore` walk it back. Recovering a lost afternoon is reading a list.
- **no stash, no detached `HEAD`, no lost branch.** A stash is a commit you cannot find; a lane's work is a
  commit `jj log` shows.
- **concurrent workspaces are first-class** (`jj workspace add`), each with its own working-copy commit, and
  `jj git push` is force-with-lease by default — so a race *refuses* the push instead of clobbering someone.

## The rules

Where these disagree with the House Rules, the House Rules win; where the House Rules name a git command, these
name its `jj` form.

1. **`jj` is the interface.** Everything that *moves a ref, writes an index, or makes a commit* goes through
   `jj`. git stays a **read-only instrument** — `git log`, `git show`, `git ls-remote`, `git cat-file`,
   `git status`, `git reflog`, `git for-each-ref` — because this repo's habit of proving a claim with a command
   is worth keeping. (The branch protection on `main` is not a git feature at all: it lives in GitHub's API and
   is read and changed with `gh`.)
2. **The refused list.** If you are about to type one of these, stop and translate: `add`, `commit`, `push`,
   `fetch`, `pull`, `merge`, `rebase`, `checkout`, `switch`, `restore`, `reset`, `revert`, `cherry-pick`,
   `stash`, `tag`, `branch`, `worktree`, `clean`, `gc`. Every one of them has a `jj` equivalent that cannot
   lose work — and mixing the two is what let 22 lanes disagree about where `main` was. `git push --force`
   stays refused outright: `jj git push` already sends force-with-lease, so a race refuses and names it instead
   of clobbering.
3. **The working copy is a commit.** House Rule 7 ("leave the kitchen clean") stops being a promise an agent
   keeps and becomes a property of the tool: there is no staging area to forget, no dirty tree to abandon, and
   `jj log` shows every lane's work.
4. **Sync by rebase, never by merge** — House Rule 1's spirit, one command:
   `jj git fetch && jj rebase -d main@origin`.
5. **Land by moving the bookmark and pushing**: `jj bookmark set main -r <rev> && jj git push --branch main`
   (or push a lane bookmark short-lived, exactly as rule 1 allows a branch).
6. **Never stash.** A stash is a commit nobody can find; `jj undo` reverses the last operation, `jj op restore
   <op>` returns the whole repo to a point in `jj op log`, and `jj op log` shows what happened, when, and by
   which host.
7. **Nothing is invisible.** `jj log -r 'all()'` is the answer to "is there work nobody can pull" that
   `recover:strand` exists to approximate today.

## SCOPE, STATED HONESTLY — what is NOT converted yet (2026-10-15)

Read this section before trusting a guard that is not listed as ported.

1. **The git hooks do not run under `jj` — ported by hand on 2026-10-15, and one of them still has no server.**
   `jj` has no hook system, so `.githooks/pre-push` (House Rule 5's `Cargo.lock --locked` check, and the `web/ui`
   wasm build behind the feature flag) and `.githooks/post-checkout` (one cargo target dir per checkout) fire for
   git only. Measured on 2026-10-15: a `jj git push --branch main` whose range **touches `web/`** returned in about
   two seconds, compiled nothing and printed no hook output. **The stand-in is `scripts/jj-pre-push.sh`, installed
   once per repo as the alias `jj push`** — an alias, not a guard: a bare `jj git push` still skips it, exactly as
   `git push --no-verify` always could. It runs the hook's checks (the lock; `pnpm ui:check`, i.e. `cargo check -p ui
   --features wasm --target wasm32-unknown-unknown`, the compile a plain workspace check cannot see) over the range
   it is about to push, keeps the same `CULEBRALUXE_*` escape hatches as House Rule 9, and passes every argument
   through to `jj git push`. **What no alias can replace:** `.githooks/pre-push` deliberately moved the *workspace*
   compile to CI on 2026-10-01, and GitHub Actions is disabled repo-wide as of 2026-10-15 — so the workspace compile
   currently runs **nowhere**. The per-checkout cargo target dir waits for item 2.
2. **The lanes are still git worktrees.** `git worktree list` is still the roster named in AGENTS.md (rule 4),
   and a lane still drives git inside its own tree — that is the one place git remains the interface, and it is
   the migration's next step (`jj workspace`). It moves as **one** change with the scripts that read
   `git worktree list` — `scripts/lane-new.sh`, `scripts/wip-snapshot.sh`, `scripts/ops/recover/*.sh`,
   `scripts/verify-checkout-build-dir.sh` — or those scripts start lying about what exists.
3. **The WIP snapshot job stays** until the last lane is converted: it is a workaround for a problem `jj` does
   not have, and retiring it before the lanes move removes a safety net the op log has not replaced in practice.
4. **`main` is protected** (`allow_force_pushes: false`, `enforce_admins: true`). An ordinary push lands. A
   rewrite of published history needs the Captain's snapshot → flip → push → restore, and that protection is
   the only thing standing between `main` and a silent clobber — it is not the obstacle, it is the guard.

## Landing on `main` under `jj`

```
jj git fetch                      # see what moved
jj rebase -d main@origin          # sync by replay, never merge
jj bookmark set main -r <rev>     # or: jj bookmark set lane/<name> -r @
jj push --branch main             # = scripts/jj-pre-push.sh + jj git push; a moved main refuses and names it
```

`jj push` is the alias `bash scripts/jj-pre-push.sh --install` writes into `.jj/repo/config.toml` (shared by every
workspace, not committed). `jj git push` still works and skips the checks — the same choice `--no-verify` always was.

A refusal is a report, not a retry (House Rule 9). Name the cause in one line and stop.

## Verification

Docs-only change to this layer: `pnpm protected-files && pnpm forge:guard-lint && pnpm forge:packet-lint`, and
`pnpm forge:sync-agents --check` whenever `AGENTS.md` moved. Handed to a lane: `pnpm forge:manifest <STORY-ID>`.
