# Handoff — the estate after a housekeeping pass (2026-10-03)

Session of 2026-10-03, agent in `src/lane-deep`. Housekeeping only: **no product file was changed**, no product
compile or test ran, and no other lane's working tree was written (the other worktrees were read with
`--no-optional-locks`). Read with `AGENTS.md` rules 1, 2, 6 and 9 open. Every row quotes the command behind it,
because a fact with no command is §5.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | Repo metadata is clean: nothing prunable, `fsck` clean, and the one `garbage` entry git reported is gone | `git worktree prune --dry-run -v` → empty; `git fsck --no-dangling` → exit 0, no output; `git count-objects -v` → `garbage: 0` (was `1`) |
| S2 | What was actually removed was one empty directory, plus Finder litter, both inside this lane | `rmdir .git/worktrees/lane-deep/refs` — empty and unreferenced, because `refs/wip/*` lives in the shared common dir; `find . -name .DS_Store` → 0 (was 1) |
| S3 | `refs/wip/*` is the rule-9 safety net, and two of its five refs belong to worktrees that no longer exist | `git for-each-ref refs/wip/` → 5. `refs/wip/ocwt` = `opencode.json`, +1 line; `refs/wip/pristine` = the whole three-tier layout move as a dirty tree. Their worktrees were `/private/tmp/ocwt` and `/private/tmp/pristine`, since removed (`HANDOFF-machine-and-lane-2026-10-03.md` S4) |
| S4 | The unreachable object surface is bigger than the ref surface, and it is clean | 229 blobs / 177 commits / 452 trees still in the pack (`git fsck --unreachable`); gitleaks over a dump of all 229 blobs → `no leaks found`, 5.20 MB scanned |
| S5 | Credential *names* appear in that dead surface; credential *values* do not | 11 of the 229 unreachable blobs match one of the six variable names listed in `docs/agent/PERIMETER.md`; `^[A-Z0-9_]+=` lines in each of the 11 → 0. The 6.8 MB one is a **WebAssembly module**, a stale `web/ui` build artifact |
| S6 | 4095 `refs/recovery/*` refs pin 4184 commits `origin/main` cannot see | `git for-each-ref refs/recovery \| wc -l` → 4095; `git for-each-ref --format='%(objectname)' refs/recovery \| git rev-list --count --stdin ^origin/main` → 4184. Sampled subjects are `cline checkpoint session=…` and `index on main: …` — and 462 distinct *authored* subjects beside them (`ENG-FORGE-DOCTOR-01: forge:doctor`, `refactor(ts): port the two dead-TS ledger gates to Rust, delete them`, the `agent/TST-SALVAGE-001/run-20261001-1` line). **4344** of these commits are referenced by no other ref on this Mac |
| S7 | 87 `refs/cline/checkpoints/*` refs remain — the namespace the 2026-09-18 exposure lived in | `git for-each-ref refs/cline/ \| wc -l` → 87; they span 2026-09-25 → 2026-10-01 |
| S8 | Only `lane/deep` exists on the remote; `lane/claude` and `lane/gpt` exist on this Mac only | `git ls-remote --heads origin \| grep lane/` → `refs/heads/lane/deep` alone; the two local tips are `692d99c7` and `6ed57f61` |
| S9 | Stranded work: 14 branches on origin carry commits main cannot see, 8 branches are local-only, 0 unpushed commits sit behind a pushed ref | `pnpm recover:strand` |
| S10 | 12 stashes hold work that exists in exactly one place | `git stash list --date=short` → 2026-08-22 … 2026-09-26 |
| S11 | A remote-tracking namespace outlives the remote it came from | `git remote` → `origin` only, yet `refs/remotes/codex/b4c2230` still resolves, and it is reachable from no local branch |
| S12 | `refs/archive/lane-deep-replay` → `692d99c7`, which is also `lane/claude`'s tip | `git rev-parse refs/archive/lane-deep-replay` |
| S13 | The main checkout is not clean: two untracked load scripts | `git -C /Users/Shared/dev/src/Culebraluxe-web status --short` → `db/loads/arm_recovery_batch_2026_10_01.sql`, `db/loads/settle_landed_candidates_2026_10_01.sql` |

One measurement trap, recorded because it produced a false `0` in this pass: **this shell is zsh**, so an unquoted
`$list` of refs is *one word*, not many — `git rev-list $tips --not $others` dies on `failed to stat '<4300 shas>'` and
the `| wc -l` after it prints `0`, which reads exactly like good news. Feed tips to stdin instead: write the pin tips and
every other ref's tips to two files, then `{ sed 's/^/^/' others; cat pins; } | git rev-list --stdin | wc -l` → **4344**
(4364 against a single `^origin/main`).

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | `refs/recovery/*` and `refs/cline/*` — 4182 refs; 4364 commits `origin/main` cannot see, **4344 referenced by nothing else** | the captain | Do not delete them, and do not run `git gc --prune=now`, without his word: they are the only reference to those 4344 commits, 462 distinct authored subjects among them (S6), and `--prune=now` has no undo (`AGENTS.md`, "PAID CODE > GIT SHA"). This pass measured that *before* touching them, and left them standing |
| H2 | the 12 stashes | the captain, and whoever wrote each one | `git stash show -p stash@{N}` before any `drop`; the drop is the one step with no undo |
| H3 | the 14 stranded and 8 local-only branches | their lane owners, named in the strand report | A branch is not mine to delete *or* to push; name it, do not tidy it. **`lane/claude` and `lane/gpt` are off limits to this lane outright** (captain, 2026-10-03: "work and touch the deep lane only; gpt and claude are off limits") |
| H4 | `refs/remotes/codex/*` | the captain | Deleting the namespace makes `b4c2230` collectable, and nothing else references it |
| H5 | the two untracked `db/loads/*.sql` in the main checkout | the captain | Not mine to commit, move or delete |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| "what exists that `main` cannot see?" | `scripts/ops/recover/strand-report.sh:1-25` | that script; `pnpm recover:strand` |
| what the `refs/wip` safety net does and does not do | `scripts/wip-snapshot.sh:1-33` | read-only |
| why checkpoint refs are a security surface | `docs/agent/postcards/2026-09-18-0819.md` | `.gitleaksignore` |
| the perimeter tools | `docs/agent/PERIMETER.md` | `pnpm scan:secrets`, `pnpm scan:deps`, `pnpm scan:migrations` |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| the first commit for this file — `git log --reverse --format=%h -- docs/agent/HANDOFF-housekeeping-estate-2026-10-03.md \| head -1` | this file, and nothing else. Landed as **`f4b55f63`** on `main` and `lane/deep` (a commit cannot contain its own sha; the later commit can, and does) | `pnpm broken:ts:sweep` → exit 0, `the tree and the inventory agree`. `git diff --check` → clean. Docs-only: the range carries no `web/`, `middle/`, `db/`, `cli/`, `forge/`, `tests/` or `Cargo.*` path, so the pre-push hook compiled nothing (`fca8bf56..f4b55f63`) |
| the second commit for this file (§7) — `git log -1 --format=%h -- docs/agent/HANDOFF-housekeeping-estate-2026-10-03.md` | S6 corrected (the pins are *not* "snapshots, not authored commits" — 4344 commits are referenced nowhere else), H1/H3 tightened with the captain's instruction, §5/§6/§7 rewritten from questions to decisions | `git diff --check` → clean; docs-only again, so no compile is owed. Same relay push form: `git fetch origin main && git rebase origin/main && git push origin HEAD:refs/heads/main HEAD:refs/heads/lane/deep` |

## 5. NOT VERIFIED — the honest gaps

- No product compile and no product test: no `cargo check`, no `pnpm slice:check`, no nextest. Nothing in the product
  changed, so no tier was owed — which also means this pass proves nothing about the build.
- The gitleaks verdict covers blob *contents*. The 177 unreachable commits and the 452 unreachable trees were counted,
  not read.
- The pinned commits were tallied, not read. S6 forbids deleting them — 4344 are referenced nowhere else and many are
  authored work — but it does not say whether they are *wanted*, and nothing here read a line of them.
- `refs/wip/ocwt` and `refs/wip/pristine` were read as diffstats, not line by line.
- `pnpm recover:strand` counts divergence against the last fetch; the fetch succeeded in this session.

## 6. OPEN — what is left, and whose it is

1. The pins stay (H1): 4344 commits exist in no other ref and 462 distinct authored subjects sit among them. If they are
   ever to go, land what is wanted first, then
   `git for-each-ref refs/recovery refs/cline --format='delete %(refname)' | git update-ref --stdin`, then `git gc --prune=now` — that last step has no undo, and `git count-objects -vH` shows what it did.
2. `lane/claude` (`692d99c7`) and `lane/gpt` (`6ed57f61`): **off limits to this lane** (captain, 2026-10-03). Their owners push them or nobody does — this pass did not.
3. Resolve the two orphan snapshots: land the diff, or `git update-ref -d refs/wip/<name>`. Finished when `git for-each-ref refs/wip/` lists only live worktrees.
4. Age the 12 stashes (H2). `preserve unrelated local work` (2026-08-24), `pre-opencode-dogfood local architect-contract` and `wip-not-mine` (both 2026-09-07) are the three nobody will ever revisit — the two that say "not mine" most of all.
5. The stale `codex` namespace (H4) and the two `db/loads/*.sql` (H5) are one-line decisions each.

## 7. DECIDED — nothing further was deleted, and why

The captain's instruction for this pass (2026-10-03): *work and touch the deep lane only; `gpt` and `claude` are off
limits* — together with *if you need to delete something, go for it*. The empty directory and the `.DS_Store` (S2) are
the whole of what was deleted, because each of the three candidates below turned out to be the only reference to
somebody's work:

- the pins: **4344 commits exist in no other ref** (S6), 462 distinct authored subjects among them, including a whole
  salvage run (`agent/TST-SALVAGE-001/run-20261001-1`, 2026-10-01) — `--prune=now` would have thrown those away for good;
- the stale `codex` ref: `b4c22302` (`fix(clients): surface reconciled relationship activity`, 2026-08-27) is referenced by
  that ref alone — `git for-each-ref --contains b4c22302` lists nothing else, and `git branch -r --contains` is empty, so it
  is on no `origin` branch either;
- the 12 stashes: a stash is by definition a state that is not `main`.

Deleting any of them on a general "go ahead" is the failure `AGENTS.md` names — a git fact voiding work that has been
paid for. Recorded, not removed. The two `db/loads/*.sql` in the main checkout (S13) are outside this lane and stay
untouched; so do the four `com.culebraluxe.*.plist` copies sitting in `build/logs`, which are launchd's, not this lane's.
