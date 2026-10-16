# The interface is `jj` — one card for the version-control layer

Written 2026-10-15 on the Captain's direction, given 2026-10-14 in his words: *"i want them to stop using git
and use just JJ"*. Until this file existed that direction lived only in a chat window. Measured the same day:
`AGENTS.md`, `CLAUDE.md`, `docs/agent/**` and all 17 `.opencode/skills/` cards mentioned `jujutsu` **zero**
times, and `git log --all -S'jujutsu'` found **no commit in any ref** that ever added the string. Ten days of
workers obeyed a handbook whose rule 2 told them to type `git pull --rebase && git push`, which is exactly what
it said. That is this repo's recurring failure in one line: **an instruction that was given and never written
down is not a rule, it is a conversation.**

The teaching companion is `docs/agent/JJ-FOR-AGENTS.md` — the git instinct, the command to type instead, and the traps
(`less` for a pager, an editor behind a bare `jj describe`). This card is the state of the conversion; that file is how
to type it.

**State, 2026-10-16: the lanes are converted to `jj` workspaces.** `jj workspace list` → 24 workspaces (`default`, the
22 `lane-*`, and the Forge sandbox `worktrees/wo/204`), `jj status` answers in every one of them, and
`docs/agent/LAYOUT.md` now describes a lane as a `jj` workspace. `scripts/lane-adopt-jj.sh` is the tool that did it and
the place its receipts live. What the conversion did **not** finish is the prose it unblocked — items 2 and 6 of
*SCOPE* below name it exactly.

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
- **no stash, no work stranded in a detached `HEAD`, no lost branch.** A stash is a commit you cannot find; a lane's
  work is a commit `jj log` shows. (`jj` does keep git's `HEAD` detached — at the **parent** of the working-copy commit,
  not at the working copy: measured across checkouts on 2026-10-16, `Culebraluxe-web` has `HEAD=2e05599e0` with
  `@=785786512a1b`, `lane-bot3` `32a92776d`/`8735056ef3ec`, `lane-mimo` `d240acf14`/`40125edf4d8d`. That is exactly why
  `git rev-parse --abbrev-ref HEAD` answers `HEAD` and a branch read is no longer how you identify a lane — and why a
  lane's uncommitted work, which lives in `@`, reaches git as *staged* changes against `HEAD`.)
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
5. **Land by moving the bookmark and pushing**: `jj bookmark set main -r <rev> && jj git push --bookmark main`
   (`--bookmark` is the documented spelling in 0.46 and `--branch` the older one that still parses — or push a lane
   bookmark short-lived, exactly as rule 1 allows a branch). `jj push --bookmark main` is the same landing with the
   pre-push checks in front of it, which is what this card recommends everywhere else.
6. **Never stash.** A stash is a commit nobody can find; `jj undo` reverses the last operation, `jj op restore
   <op>` returns the whole repo to a point in `jj op log`, and `jj op log` shows what happened, when, and by
   which host.
7. **Nothing is invisible.** `jj log -r 'all()'` is the answer to "is there work nobody can pull" that
   `recover:strand` exists to approximate today.

## SCOPE, STATED HONESTLY — what is NOT converted yet (2026-10-15, re-checked 2026-10-16)

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
2. **CLOSED 2026-10-16 — the lanes are `jj` workspaces, and this card's largest item is done.** `jj workspace list`
   → **24** (`default` + the 22 `lane-*` + the Forge sandbox `worktrees/wo/204`), `git worktree list` → the same 24,
   `jj status` answers in a lane (`The working copy has no changes.`), and the alias is there too
   (`jj -R ../lane-claude config get aliases.push`). The old reading, replaced: *"The lanes are still git worktrees, and
   today `jj` cannot run in one at all"* — `jj status` inside `lane-claude` answered *There is no jj repo in "."*. The
   conversion was `scripts/lane-adopt-jj.sh` (`--all`, one at a time, `--dry-run`, a kept-copy path), and it verified
   what it did: every `refs/heads/*` ref unmoved, `lane-claude-deploy`'s detached HEAD still at `d240acf14`, the kept
   copy for each lane content-identical to what the workspace loaded (`rsync -rcn`; only mtimes differ), and no
   `.jj-adopt-*` directory left behind. All ten worktree-aware scripts — `scripts/lane-new.sh` (which creates a `jj`
   workspace now), `scripts/wip-snapshot.sh`, `scripts/lane-cargo-config.sh`, `scripts/verify-checkout-build-dir.sh`,
   `scripts/agent-worker-once.sh`, `scripts/opencode-session-stop.sh`, `scripts/restructure-domain-layout.sh`,
   `scripts/ops/recover/relay.sh`, `scripts/ops/recover/unblock.sh`, `scripts/ops/recover/strand-report.sh` — still see
   the lane, because `jj workspace add` inside a colocated repo registers a git worktree as well. **What the conversion
   did not do is item 6 below:** six control-plane scripts still read a lane's branch the git way, and a lane answers
   empty now (`git branch --show-current` → `""`, `git rev-parse --abbrev-ref HEAD` → `HEAD`, measured in `lane-claude`
   and `lane-bot3`). A lane's working tree was recreated, so its dirty files had to become commits or belong to the
   snapshot first; they are held in `refs/wip/*` and pushed as `salvage/wip/*`, and the conversion snapshotted the
   previously-untracked files of five lanes into `@` — which is what `jj` does with a working copy, not a loss.
3. **The WIP snapshot job is now redundant and is still installed.** Its own trigger was *"stays until the last lane is
   converted"*, and on 2026-10-16 the last lane was: all 24 workspaces are `jj` workspaces, so the condition is met.
   `launchctl list` still shows `com.culebraluxe.wip-snapshot` running with exit 0, and retiring it is a deliberate
   step rather than a consequence of the conversion — what replaces it is `jj op log` plus the fact that a lane's
   working copy is a commit, and those two are the reason it was written as a workaround in the first place. Until
   somebody retires it, it is a second safety net and not a lie; what it must not become is a reason to leave work
   uncommitted, because `refs/wip/*` is a private ref nobody pulls.
4. **`main` is protected** (`allow_force_pushes: false`, `enforce_admins: true`). An ordinary push lands. A
   rewrite of published history needs the Captain's snapshot → flip → push → restore, and that protection is
   the only thing standing between `main` and a silent clobber — it is not the obstacle, it is the guard.
5. **The agent config names `jj` now — both halves** (closed 2026-10-15, hours after it was measured as *zero*). One
   renderer, `forge/src/engine/opencode_agents.rs`, is the only place either half is written: the repo-root
   `opencode.json` is generated from it (`pnpm forge:opencode-config`, drift-checked inside `pnpm forge:harness`) and a
   live turn is handed the same document as `OPENCODE_CONFIG_CONTENT` (`v2_agent_env`), so a human session in this
   checkout and a Forge turn cannot disagree. **Granted:** `INSPECT_SHELL` — and the test analyst's list beside it — now
   carry `jj status`, `jj diff`, `jj log`, `jj show` and `jj bookmark list`, so a read-only Forge role can run the
   commands rule 2, rule 10 and this card tell it to run. **Denied:** `PUBLICATION_DENY` now names `jj push`,
   `jj git push`, `jj git fetch`, `jj git remote`, `jj git clone` and `jj util` beside their git spellings — `jj util`
   deliberately, because `jj util exec -- …` is how the `jj push` alias itself is defined (in the repo's config file —
   `jj config path --repo` → `~/.config/jj/repos/d0acdad294aa332619a6/config.toml` on this machine, measured 2026-10-16;
   not `.jj/repo/config.toml`, which is a pointer file in a lane) and
   is therefore the general route around that whole list. Two tests hold the halves in `tests/tests/opencode_v2_agents.rs`:
   `v2_the_jj_spelling_of_a_denied_git_verb_is_denied_too` (the pairing — a git verb added without its `jj` spelling
   fails) and `v2_read_only_roles_inspect_with_jj_and_only_inspect` (the grant, plus that no *other* `jj` verb is
   allowed to a read-only role). **What a deny list is not:** the vendor matches command *prefixes*, so `env git push`
   and `bash -c "git push"` were never covered and are not now; the durable control is the one §2/§20 always rested on
   — publication as Forge's own code path (`forge/src/engine/git_publish.rs`, still git on purpose, still not a model's
   shell). **The reason the git entries in `INSPECT_SHELL` were kept expired on 2026-10-16:** they stayed *"until the
   lanes convert (item 2), because a lane has no `.jj` to inspect"*, the lanes converted, and `git status`, `git diff`
   and `git log` are still granted beside their `jj` spellings for no reason that survives. The same is true of the
   packet instruction an implementer reads — *"Create a local git commit with the intended changes when the story
   requires it"* (`forge/src/engine/packet.rs:338`), a sentence that was TRUE in a lane and is **false in every
   workspace now**. Both are deliberately not part of the conversion: each changes what a Forge turn may do, and each
   carries the harness gates and the two tests in `tests/tests/opencode_v2_agents.rs` with it.

6. **Six control-plane scripts read a lane's branch the git way, and a lane answers empty now (found 2026-10-16, by the
   conversion).** `git branch --show-current` → `""` and `git rev-parse --abbrev-ref HEAD` → `HEAD`, measured in
   `lane-claude` and `lane-bot3` — and true of the main checkout since it became colocated, which is why this is a bug
   class the conversion *spread* from one checkout to 23 rather than created. `scripts/ops/recover/unblock.sh` and
   `scripts/agent-worker-once.sh` already read a checkout the `jj`-aware way and say so in a comment; these six do not:
   `scripts/ops/gate/slice-check.sh` (its receipt line prints `detached` — cosmetic),
   `scripts/forge-v10-release-receipt.sh`, `scripts/release-record.sh` (two sites), `scripts/vercel-deploy-prod.sh`,
   `scripts/vercel-release-prod.sh`, and `scripts/vercel-provision-rust-project.sh`, which compares the branch to `main`
   and therefore **refuses** to run — fail-closed, which is the right direction for a production cutover. None of the six
   is a lane tool, and the four release/deploy ones run from the control plane, so today this is a report and not an
   outage. The `jj` spellings that replace a branch read: for *which checkout is this*, the **workspace name** —
   `jj workspace root` (→ `/Users/Shared/dev/src/lane-bot3`) or the `NAME: path` column of `jj workspace list`, which is
   the reading a lane-named script actually wants; for *where does this lane's work sit*,
   `jj --no-pager log -r @- --no-graph -T 'bookmarks'` (`@` carries none — it is the empty working-copy commit). Plain
   `jj bookmark list` is **not** an identifier: run in a lane it prints every bookmark in the repository.

## Landing on `main` under `jj`

```
jj git fetch                      # see what moved
jj rebase -d main@origin          # sync by replay, never merge
jj bookmark set main -r <rev>     # or: jj bookmark set lane/<name> -r @
jj push --bookmark main           # = scripts/jj-pre-push.sh + jj git push; a moved main refuses and names it
```

`jj push` is the alias `bash scripts/jj-pre-push.sh --install` writes into the repository's config file — the path
`jj config path --repo` prints (`~/.config/jj/repos/<repo-id>/config.toml`), shared by every workspace and outside every
checkout, so it is not committed. Measured 2026-10-16: a lane answers the same path, and
`jj -R ../lane-claude config get aliases.push` prints the alias, so `jj push` is available in a lane too. `jj git push`
still works and skips the checks — the same choice `--no-verify` always was.

A refusal is a report, not a retry (House Rule 9). Name the cause in one line and stop.

## Verification

Docs-only change to this layer: `pnpm protected-files && pnpm forge:guard-lint && pnpm forge:packet-lint`, and
`pnpm forge:sync-agents --check` whenever `AGENTS.md` moved. Handed to a lane: `pnpm forge:manifest <STORY-ID>`.
