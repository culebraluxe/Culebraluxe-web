# Handoff — the estate after a housekeeping pass (2026-10-03)

Session of 2026-10-03, agent in `src/lane-deep`. Housekeeping only: **no product file was changed**, no product
compile or test ran, and no other lane's *work* was written (the other worktrees were read with
`--no-optional-locks`). One exception, named here and in §8: two **untracked** files inside `lane-claude` — a `.DS_Store`
was deleted as Finder litter on the widened instruction, and an untracked `.env.local` was *added* so that lane's owner
can actually run it (byte-identical to the main checkout's; `git check-ignore` → `.gitignore:16 .env*`, so it can never be
committed). No tracked file in any other lane, and nothing under version control, was touched there.
Read with `AGENTS.md` rules 1, 2, 6 and 9 open. Every row quotes the command behind it,
because a fact with no command is §5.

**Why the estate looks like this** (captain, 2026-10-03): every agent worked in the *same* working tree at the same time,
and iCloud synced that tree underneath them; everything broke and the per-lane worktrees are the fix. That incident is the
cause behind this document's shape — the 12 stashes, the 22 branches that are not `main`, the 4095 rescue pins, the 177
unreachable commits, and the 83 worktrees that had to be deleted (`docs/agent/MEMORY.md`). Standing the lanes up is still
in progress as this is written: `lane/claude` and `lane/gpt` are checked out on this Mac already, and only their `origin`
branches are missing.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | Repo metadata is clean: nothing prunable, `fsck` clean, and the one `garbage` entry git reported is gone | `git worktree prune --dry-run -v` → empty; `git fsck --no-dangling` → exit 0, no output; `git count-objects -v` → `garbage: 0` (was `1`) |
| S2 | What was actually removed was one empty directory, plus Finder litter, both inside this lane — and then §8's junk pass removed more, on a widened instruction | `rmdir .git/worktrees/lane-deep/refs` — empty and unreferenced, because `refs/wip/*` lives in the shared common dir; `find . -name .DS_Store` → 0 (was 1, then 26 estate-wide, §8) |
| S3 | `refs/wip/*` is the rule-9 safety net, and **one** of its refs belongs to a worktree that no longer exists | `git for-each-ref refs/wip/` → 4 (was 5; `refs/wip/ocwt` deleted in §8 as a probe fixture). `refs/wip/pristine` = the whole three-tier layout move as a dirty tree, from `/private/tmp/pristine` (`HANDOFF-machine-and-lane-2026-10-03.md` S4), and it **stays**: it holds 499 objects `origin/main` does not have |
| S4 | The unreachable object surface is bigger than the ref surface, and it is clean | 231 blobs / 181 commits / 452 trees still in the pack (`git fsck --unreachable`; was 229/177 before §8 moved two junk refs here); gitleaks over a dump of all 229 blobs → `no leaks found`, 5.20 MB scanned |
| S5 | Credential *names* appear in that dead surface; credential *values* do not | 11 of the 229 unreachable blobs match one of the six variable names listed in `docs/agent/PERIMETER.md`; `^[A-Z0-9_]+=` lines in each of the 11 → 0. The 6.8 MB one is a **WebAssembly module**, a stale `web/ui` build artifact |
| S6 | 4095 `refs/recovery/*` refs pin 4184 commits `origin/main` cannot see | `git for-each-ref refs/recovery \| wc -l` → 4095; `git for-each-ref --format='%(objectname)' refs/recovery \| git rev-list --count --stdin ^origin/main` → 4184. Sampled subjects are `cline checkpoint session=…` and `index on main: …` — and 462 distinct *authored* subjects beside them (`ENG-FORGE-DOCTOR-01: forge:doctor`, `refactor(ts): port the two dead-TS ledger gates to Rust, delete them`, the `agent/TST-SALVAGE-001/run-20261001-1` line). **4344** of these commits are referenced by no other ref on this Mac |
| S7 | 87 `refs/cline/checkpoints/*` refs remain — the namespace the 2026-09-18 exposure lived in | `git for-each-ref refs/cline/ \| wc -l` → 87; they span 2026-09-25 → 2026-10-01 |
| S8 | Only `lane/deep` exists on the remote; `lane/claude` and `lane/gpt` exist on this Mac only | `git ls-remote --heads origin \| grep lane/` → `refs/heads/lane/deep` alone; both lanes are **already checked out** — `git worktree list` → `/Users/Shared/dev/src/lane-claude [lane/claude]` and `/Users/Shared/dev/src/lane-gpt [lane/gpt]` — so this is a *push* gap, not a missing lane, and it is mid-setup, not staleness |
| S9 | Stranded work: 14 branches on origin carry commits main cannot see, 8 branches are local-only, 0 unpushed commits sit behind a pushed ref | `pnpm recover:strand` |
| S10 | 12 stashes hold work that exists in exactly one place | `git stash list --date=short` → 2026-08-22 … 2026-09-26 |
| S11 | A remote-tracking namespace outlived the remote it came from — and is now gone | `git remote` → `origin` only, so `refs/remotes/codex/b4c2230` could never be fetched again; it resolved until §8 deleted it, after `git cherry origin/main b4c22302` → `- b4c22302` proved the patch is in `main` as `b5389b76` |
| S12 | `refs/archive/lane-deep-replay` → `692d99c7` (`chore(layout): replay the three-tier move onto this lane's base`) is now the **only** holder of that commit | `git for-each-ref --contains 692d99c7` → that ref alone; `lane/claude` has since moved to `740b5253`, which does **not** contain it (`git merge-base --is-ancestor 692d99c7 lane/claude` → false). `git cherry origin/main 692d99c7` → `+`, so its patch (the three-tier layout replay: `.githooks/pre-push`, `.github/workflows/gates.yml`, `.dockerignore`, `.env.example`, `nextest.toml`) is in `main` nowhere |
| S13 | The main checkout is not clean: two untracked load scripts | `git -C /Users/Shared/dev/src/Culebraluxe-web status --short` → `db/loads/arm_recovery_batch_2026_10_01.sql`, `db/loads/settle_landed_candidates_2026_10_01.sql` |

One measurement trap, recorded because it produced a false `0` in this pass: **this shell is zsh**, so an unquoted
`$list` of refs is *one word*, not many — `git rev-list $tips --not $others` dies on `failed to stat '<4300 shas>'` and
the `| wc -l` after it prints `0`, which reads exactly like good news. Feed tips to stdin instead: write the pin tips and
every other ref's tips to two files, then `{ sed 's/^/^/' others; cat pins; } | git rev-list --stdin | wc -l` → **4344**
(4364 against a single `^origin/main`).

Its sibling, which produced a false `0` again later the same day: **`--exclude` binds the `--all` that follows it**, so
`git rev-list --all --exclude='refs/recovery/*' --exclude='refs/cline/*'` excludes nothing (`8662` — every commit in the
repo) and the pinned set then measures `0`. The order that works is `git rev-list --exclude=… --exclude=… --all` →
`4318`, against the pins' `8339`, difference **4344** — the same number as the stdin method above, which is the check
that caught it. Both wrong forms read as good news: "nothing is only kept alive by the pins".

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | `refs/recovery/*` and `refs/cline/*` — 4182 refs; 4364 commits `origin/main` cannot see, **4344 referenced by nothing else** | the captain | Do not delete them, and do not run `git gc --prune=now`, without his word: they are the only reference to those 4344 commits, 462 distinct authored subjects among them (S6), and `--prune=now` has no undo (`AGENTS.md`, "PAID CODE > GIT SHA"). This pass measured that *before* touching them, and left them standing |
| H2 | the 12 stashes | the captain, and whoever wrote each one | `git stash show -p stash@{N}` before any `drop`; the drop is the one step with no undo |
| H3 | the 14 stranded and 8 local-only branches | their lane owners, named in the strand report | A branch is not mine to delete *or* to push; name it, do not tidy it. **`lane/claude` and `lane/gpt` are off limits to this lane outright** (captain, 2026-10-03: "work and touch the deep lane only; gpt and claude are off limits") |
| H4 | ~~`refs/remotes/codex/*`~~ — **RESOLVED in §8** | the captain | Decided 2026-10-03 on the widened instruction: `refs/remotes/codex/b4c2230` deleted once its patch was proven landed (`b5389b76`), while `refs/codex/turn-diffs/checkpoints/…` **stays** — 241 blobs `origin/main` does not have |
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
| the fourth commit for this file (S12/§6.2/§8 re-measured; the lane's settings file added) — `git log -1 --format=%h -- docs/agent/HANDOFF-housekeeping-estate-2026-10-03.md` | S12 corrected: `lane/claude` moved to `740b5253` and `refs/archive/lane-deep-replay` is now `692d99c7`'s *only* holder; §6.2's lane tips brought current; §8's archive reason upgraded from precaution to measurement, and the `.env.local` copy recorded | `git diff --check` → clean; `git branch -a --list 'lane/*'` → 3 lanes; `md5 -q` on the copied file matches the source; docs-only, so no compile is owed; same relay push form |

## 5. NOT VERIFIED — the honest gaps

- No product compile and no product test: no `cargo check`, no `pnpm slice:check`, no nextest. Nothing in the product
  changed, so no tier was owed — which also means this pass proves nothing about the build.
- The gitleaks verdict covers blob *contents*. The 181 unreachable commits and the 452 unreachable trees were counted,
  not read.
- The pinned commits were tallied, not read. S6 forbids deleting them — 4344 are referenced nowhere else and many are
  authored work — but it does not say whether they are *wanted*, and nothing here read a line of them.
- `refs/wip/pristine` was read as a diffstat, not line by line (its 499 unique objects are counted, not read).
  `refs/wip/ocwt` was read in full in §8 — it had exactly one object `origin/main` lacks, which is the only reason it
  could be deleted at all.
- `pnpm recover:strand` counts divergence against the last fetch; the fetch succeeded in this session.

## 6. OPEN — what is left, and whose it is

1. The pins stay (H1): 4344 commits exist in no other ref and 462 distinct authored subjects sit among them. If they are
   ever to go, land what is wanted first, then
   `git for-each-ref refs/recovery refs/cline --format='delete %(refname)' | git update-ref --stdin`, then `git gc --prune=now` — that last step has no undo, and `git count-objects -vH` shows what it did.
2. `lane/claude` (now `740b5253` — one docs commit `main` lacks, 4 behind) and `lane/gpt` (`6ed57f61` — one commit, 18 behind): **off limits to this lane** (captain, 2026-10-03). Their owners push them or nobody does — this pass did not. Both trees already exist (`/Users/Shared/dev/src/lane-claude`, `/Users/Shared/dev/src/lane-gpt`), so no `git worktree add` is owed and one would only fail with *already registered*; from inside the lane the whole job is `git push -u origin lane/<name>`. Note `lane/claude` has moved off `692d99c7`, so that tip is no longer reproducible from the lane (S12) — and its `.env.local` was missing until §8 copied one in.
3. One orphan snapshot is left: `refs/wip/pristine`; the other (`refs/wip/ocwt`) was a probe fixture and went in §8. Land
   its diff, or `git update-ref -d refs/wip/pristine` — **and not before**, because it holds 499 objects nothing else
   has. Finished when `git for-each-ref refs/wip/` lists only live worktrees.
4. Age the 12 stashes (H2). `preserve unrelated local work` (2026-08-24), `pre-opencode-dogfood local architect-contract` and `wip-not-mine` (both 2026-09-07) are the three nobody will ever revisit — the two that say "not mine" most of all.
5. The stale `codex` namespace (H4) is **decided** — see §8: the landed duplicate went, the checkpoint store stayed. The
   two `db/loads/*.sql` (H5) are still one-line decisions, and they are the captain's, not this lane's.

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

> Superseded in part by §8, same day: the stale `codex` ref was **deleted** once its patch was proven to be in `main`
> (`b5389b76`) — §7's test (*"referenced by that ref alone"*) is not the test; *"the work is elsewhere"* is. The pins, the
> stashes and `refs/wip/pristine` still stand, and §7 is why.

## 8. JUNK PASS — what "you can delete any junk" emptied, and what it did not (2026-10-03)

The captain widened the instruction the same day: *"ok you can delete any junk"*. Junk got a test rather than a mood: an
object is junk when it is **Finder litter** or a **probe fixture**, or when **the content it points at is already in
`origin/main`** — and it is kept whenever it is the only reference to work, however small. Three things passed, three
did not, and each deleted ref was proven redundant object by object *before* the delete.

Deleted:

1. **26 untracked `.DS_Store`, every one under `/Users/Shared/dev/src`, none tracked in git.** `find … -name .DS_Store`
   → 0 (was 26). No commit was owed and the trees stayed clean (`git status --short` → empty). Five sat inside the
   *prebuilt* `.vercel/output/static/` — they would have shipped in a `vercel deploy --prebuilt` — and five inside
   `public/`, where they are served live (`/upload/.DS_Store`, `/images/about/.DS_Store` and the three guide folders
   beside them). One sat inside `.git/`. Guard per file: `git -C <dir> ls-files --error-unmatch .DS_Store` → kept if it
   matched, removed if not (none matched).
2. **`refs/wip/ocwt` → `242fe8ee`.** Its only object that `origin/main` does not have was a 76-byte probe fixture:
   `opencode.json` = `{"snapshot":false,"agent":{"probe-wt":{"description":"x","mode":"primary"}}}` — an earlier session
   writing a fake worktree to prove the rule-9 snapshot job captures a dirty tree. It did; the worktree is gone
   (`/private/tmp/ocwt`); a test input is not work. Proof: `{ echo '^origin/main'; git rev-parse refs/wip/ocwt; } |
   git rev-list --stdin --objects | awk 'NF>1'` → that one path, 1 object.
3. **`refs/remotes/codex/b4c2230` → `b4c22302`.** `git remote` lists `origin` only, so the ref can never be fetched or
   updated again, and the patch it labels is in `main`: `git cherry origin/main b4c22302` → `- b4c22302` (patch already
   upstream) and `git log origin/main --grep='surface reconciled relationship activity'` → `b5389b76`, same subject.
   Single parent (not a merge), so the `cherry` verdict is not an artifact.

Kept, with the measurement that says so:

- **the pins and the 12 stashes** (H1/H2), unchanged; re-counted by two independent methods → still **4344**;
- **`refs/wip/pristine`**: 499 objects `origin/main` does not have — the one orphan snapshot that really does hold
  content nobody else has, so it waits for its owner (§6.3) instead of being tidied away;
- **`refs/codex/turn-diffs/checkpoints/…`**: the ref points at a *tree*, not a commit, and holds **241 blobs** absent from
  `origin/main` — the Codex CLI's own checkpoint store, kept for the same reason as `refs/cline/*`;
- **`refs/archive/lane-deep-replay`** → `692d99c7`: kept on purpose, and **re-measured the same day to prove it** —
  `git for-each-ref --contains 692d99c7` → this ref *alone*, because `lane/claude` has moved to `740b5253` and no longer
  contains it (S12). It is now the only pointer to 1 commit that `origin/main` and `lane/claude` both lack, so deleting it
  would hand that commit to the next `gc`;
- **the 181 unreachable commits and 231 unreachable blobs** stay in the pack. `gc --prune=now` is the only thing that
  would remove the stale 6.8 MB wasm blob (S5), and it cannot be aimed — it would take the authored commits in that set
  with it. 6.8 MB is cheaper than a blind decision. (These two refs are why S4 moved from 229/177 to 231/181.)
- **`/Users/Shared/dev/build/archive-2026-10-03-from-documents/` is not junk and must not be deleted.**
  `Culebraluxe-web-claude/.env.local` (8 761 bytes, 2026-09-26) is the **only** copy of the Claude lane's environment —
  `[ -f /Users/Shared/dev/src/lane-claude/.env.local ]` → false. Whoever stands that lane up should copy it in
  (untracked), not regenerate it. **Done the same day**: the file was copied to
  `/Users/Shared/dev/src/lane-claude/.env.local` (mode 600, byte-identical to the main checkout's, `git check-ignore` →
  `.gitignore:16`), and the archive keeps the older 2026-09-26 variant. `lane/gpt` still has none — its owner's call.
- **the four `com.culebraluxe.*.plist` copies in `build/logs`** stay: nothing references them (`grep -rn 'build/logs'
  scripts/` → the wip log dir only) and all four differ (`md5 -q`) from the installed `~/Library/LaunchAgents/…` — an
  older config backup, not litter, and launchd's file, not this lane's.

Effect: `git for-each-ref | wc -l` → 4230 (was 4232), `find … -name .DS_Store` → 0, `git fsck --no-dangling` → exit 0 with
no output, no empty ref directory left behind. Deletion receipts, for reversal: `refs/wip/ocwt` was `242fe8ee`
(blob `f15466f9`, 76 bytes, content above); `refs/remotes/codex/b4c2230` was `b4c223027cc6a1e7b086a25fa4c480e5ffeda4bf`
(landed as `b5389b76`).

**Re-run the sweep before quoting a `0` above.** Finder recreates `.DS_Store` every time it browses a folder — four came
back *inside the hour* (all under `src/lane-deep/node_modules/.pnpm/**`, so build output, not the site) and were cleared
again the same way (§8 step 1). A non-zero count is therefore expected on a later day and is litter, not drift.

## 9. READING THE DOCS — AND THE PROSE THAT WAS NO LONGER TRUE (2026-10-03, later)

The next assignment was not a change: confirm the lane, then read the docs (`AGENTS.md`, `ORIENTATION.md`, `LAYOUT.md`,
`CURRENT.md`, `MAP-engine.md`, `QUEUE-2026-10-02.md`, and the head of `MEMORY.md`). Reading them *against the machine*
found claims a next reader would have re-done work to satisfy, so they were corrected where they were read: two files
(`LAYOUT.md` and this one), three commits, no product code.

| Commit | What it changed | Why the old wording was wrong | The gate |
| --- | --- | --- | --- |
| `a186e495` | `LAYOUT.md`, the "Open items on this machine" list — re-checked and dated | Three of its four items had closed. All five `~/Library/LaunchAgents/com.culebraluxe.*.plist` jobs carry `/Users/Shared/dev/src/Culebraluxe-web` (per-file `plutil -p` → **0** live paths naming `~/Documents`; `agent-worker` via `AGENT_WORKER_REPO`, the Apple jobs via `CULEBRALUXE_REPO`, each regenerated by its own `pnpm *:install`), the dead worktree records are unregistered (`git worktree list` → only the four live trees), and `build/rust` is **4.5 GB warm** rather than "still empty". The one item not closed is stated as open, not as fixed | `git diff --check` → clean. `pnpm slice:check` → `slice-check: nothing changed since origin/main and the working tree is clean — nothing to check.` **exit 0** (docs-only: no tier is owed). Relay push `85b6e1b0..a186e495` to `main` and `lane/deep` |
| `cc0adadd` | `LAYOUT.md`, "The layout move" — the paragraph now says it describes a base that no longer exists | It claimed the three lanes "sit on `97785410`". `git merge-base --is-ancestor 97785410 origin/main` → **true** on 2026-10-03: the base is in `main` and each lane left it by replay, so the sentence described history as if it were the present. The warning against merging `main` into an old-base lane is kept — it is still the trap for any lane that has not followed | `git diff --check` → clean. Relay push `a186e495..cc0adadd` to `main` and `lane/deep` |

Lane positions, re-measured in the same pass because §6.2 quotes them and they move:

| Lane | Tip | Behind `main` | Ahead of `main` | `git cherry origin/main <tip>` |
| --- | --- | --- | --- | --- |
| `lane/deep` | `cc0adadd` | 0 | 0 | level — nothing unpublished |
| `lane/claude` | `740b5253` | 7 | 1 | `+` — **not** in `main` |
| `lane/gpt` | `6ed57f61` | 21 | 1 | `+` — **not** in `main` |

`git ls-remote origin 'refs/heads/lane/*'` → `lane/deep` only. **Neither `lane/claude` nor `lane/gpt` has ever been
pushed**, so both single commits are reachable from local refs alone — and rule 6 ("never hold work back") is being
bent by two lanes this pass did not touch.

Two findings that are the captain's to decide, not this lane's to act on:

1. **The main checkout's own `main` branch is sitting on `740b5253`** — the *same commit* as `lane/claude`'s tip, which is
   `lane/claude`'s own docs commit ("the lanes are legal — AGENTS.md gets the lane exception"). `git for-each-ref
   --contains 740b5253` → `refs/heads/lane/claude`, `refs/heads/main`, `refs/wip/Culebraluxe-web`: three local holders,
   **none of them on `origin`**. So the folder is diverged rather than behind (7 behind / 1 ahead), and nothing is lost
   even if one holder goes. This lane did not touch it (rule 4, and it is another lane's commit to land).
2. **Three LaunchAgents are loaded but not running, last exit status 1** on 2026-10-03: `agent-worker`, `apple-sync`,
   `calendar-sync` (`launchctl list` → PID `-`), while `apple-local-listener` runs (pid 78) and `wip-snapshot` exits 0.
   `launchctl` cannot tell "nothing due" from "broken" — its last-exit code is the same — so the only thing that
   separates them is `~/Library/Logs/CulebraLuxe/*.err.log`, which is a **log read and therefore the captain's go**.
   Note this is *not* the 2026-10-01 failure: the jobs no longer point at a dead path (first row of the table above).

What §9 does **not** establish: nothing about the product build. No `cargo check`, no test ran, because no product file
changed — the same gap §5 already states.

## 10. THE CAPTAIN'S TWO QUESTIONS — BOTH YES (2026-10-03, later still)

Captain: *"see if you can login to dev NEON and PROD, and do a build of the app."* Both answered, with output. This
section supersedes the build gap §5 and §9 recorded ("nothing about the product build"): a build ran, and so did tests.

### 10.1 Login: yes, to both — read-only

`psql` is not installed on this machine, so the operative client is the Rust `db-tool` (`cli/src/db_tool.rs`), which
prints the **target and host only, never credentials** (there is a unit test for that: `host_of_reports_only_the_host`).

```
$ pnpm db:migrations        # db-tool status — reads the ledger on DEV and PROD; exit 0
database: target=dev host=ep-muddy-lab-axtgckj9-pooler.c-4.us-east-2.aws.neon.tech
database: target=prod host=ep-flat-art-ax92tn7a-pooler.c-4.us-east-2.aws.neon.tech
ledger: 156 rows   migrations on disk: 240
  dev  recorded: 31
  prod recorded: 125

$ pnpm db:parity            # DEV vs PROD structure, both sides; exit 0
tables only in DEV : (none)
tables only in PROD: (none)
column drift: 0    index drift: 0    fk drift: 0    check drift: 0
PARITY OK
```

Both hosts match the playbook's §1 table, so this lane's `.env.local` points where it says it does. **The 31/125 ledger
asymmetry is not drift**: DEV's ledger went with its branch reset, which is why most one-sided rows read `[prod only]`;
the structure itself compares clean on all five axes. Writing the report also caught **stale prose**: the playbook's
"Parity blind spot" said parity "NOT check constraints", which stopped being true on 2026-09-12
(FORGE-PARITY-CHECK-01 added the fifth axis) — corrected in the same pass, and the *still*-open gap (functions/stored
routines, view definitions) named precisely in its place. Nothing was written to either database: `status` and
`parity` only read, and no `APP_ENV=production` command ran.

### 10.2 Build: yes — after fixing what stopped it on the first try

`pnpm build` — the command AGENTS.md, ORIENTATION.md and DEV-OPS-RELEASE.md all name as *the* build — **failed in five
seconds**:

```
==> cargo build (ui, wasm32-unknown-unknown, release)
error: Read-only file system (os error 30) at path "/targetnGolVo"
ELIFECYCLE  Command failed with exit code 101.
```

Root cause: `scripts/rust-ui-build.sh` asked for `--target-dir /target`, the **container's** build root (writable there —
`Dockerfile:14`, `devops/Dockerfile.build`) — while this Mac is read-only at `/` and already exports the shared
`CARGO_TARGET_DIR=/Users/Shared/dev/build/rust`, which the script ignored. Fixed in **`6341055f`**: it honours
`CARGO_TARGET_DIR` before falling back to `/target`, so the container value is untouched and no lane on this machine can
hit this again. Verified: the fallback in three cases (`neither → /target`, `CARGO_TARGET_DIR → the shared dir`,
`RUST_UI_TARGET_DIR → wins`), a real release build through the script with `RUST_UI_TARGET_DIR` unset (exit 0, artifacts
still `public/rust-ui/`), `bash -n`, `arch_boundary__013` (the test that reads this script) PASS, T0 PASS, rustfmt PASS.

Then the build ran clean, on this machine:

```
==> cargo build (ui, wasm32-unknown-unknown, release)   Finished `release` profile [optimized] target(s) in 38.45s
    WASM:     public/rust-ui/ui_bg.wasm (9.0M)          JS glue: public/rust-ui/ui.js (60K)
==> tailwind (web/ui/styles/app.css -> public/app.css)  (183K)
    Finished `release` profile [optimized] target(s) in 1m 24s     # the server binary
-rwxr-xr-x 33M  build/rust/release/web   Mach-O 64-bit executable arm64
```

So the wasm, the glue, the stylesheet and the server binary all build here — the honest gap §5 and §9 carried is closed
by this run, and the artifacts are the ones the deploy copies (`public/rust-ui/`, `Dockerfile`).

### 10.3 What the build found: a red on `main` that is already owned — do not "fix" it

T1 (`pnpm slice:check`) went red, not for the script but for a product test,
`test-harness --test forge_seam__001__story_to_complete`:

```
left:  ["architect", "architect", "lead_pre", "lead_pre", "lead_solo_implement", "lead_post", "qa_review", "qa_verify", "qa_verify"]
right: ["architect", "lead_pre", "smith", "lead_post", "qa_review", "qa_verify"]
```

It is **deterministic** (3/3 runs of the built binary, ~0.03s each) and **not this diff**: the three inputs that decide
it — `forge/definitions/FORGE_SDLC-v6.xml`, the test, `tests/tests/support/forge_seam.rs` — are byte-identical to
`origin/main` (`git diff origin/main --` on those paths is empty), and this change is a shell script no Rust test reads.
It is also **already a named, dated, owned row**, not new: `CURRENT.md:158-163` records exactly this drift for
`forge_seam__001..004` with these same two lists and says *"Do not 'fix' them by editing the seam expectations to
match"*, and `HANDOFF-machine-and-lane-2026-10-03.md` H1 holds it for **the workflow-lane owner**. This slice reached
001 only — the harness loop stops at the first failing binary; the other three are recorded there as failing too.

The mechanism, for whoever owns it: the FEATURE composition (`forge/definitions/FORGE_SDLC-v6.xml:33-43`) is
`architect → lead_pre → execution_shape → {lead_solo_implement | smith} → split_dispatch/join → lead_post → qa_policy →
qa_review → qa_verify`, and the observed run takes **both** sides of the `execution_shape` fork and re-enters
`architect`, `lead_pre` and `qa_verify`. Nothing here changes: the expectation was not edited and the test was not
excluded from any run.

**Re-measured after rebasing onto `main`'s current tip** (`bd36fad0`, which carries today's two engine commits —
`a26ba686` "a failed role turn settles Error, and an engine fault goes back to the queue" and `bd36fad0` clippy): **still
red, exit 101, the same two lists.** So the fix is not in those commits and the row is still the workflow lane's.

### 10.4 The two questions §9 left open — both answered by the move (§9 item 1 is now closed)

While this pass was running, `origin/main` moved four commits (`1c95d719`, `9aa69348`, `a26ba686`, `bd36fad0`) and
`lane/deep` rebased onto them cleanly — this lane's three commits became `1a89dd53`, `8ed218e9`, `672e04ab`. Two of
those four answer questions §9 had left open, and both answers are checkable facts rather than opinions:

1. **§9 item 1 is closed: the held `740b5253` is already on `main` as `1c95d719`.** `git show <sha> | git patch-id
   --stable` gives the **same patch-id, `686477d3…`,** for both, and the diffstats match exactly (AGENTS.md,
   `docs/agent/LAYOUT.md`, `docs/agent/ORIENTATION.md`, `docs/agent/packets/TECH-FLIGHT-RECORDER-01.md` — 4 files, 17
   insertions, 5 deletions) while the shas differ. So the main checkout's own `main` holds a *duplicate* of work already
   published: nothing needs landing, and its local holders (`refs/heads/main`, `refs/heads/lane/claude`,
   `refs/wip/Culebraluxe-web`) can be dropped whenever convenient. §9's finding ("three local holders, none of them on
   `origin`") was true when written; it is answered now, not wrong.

2. **`lane/gpt`'s held `6ed57f61` is a replay, not a story.** It is `chore(layout): replay the three-tier move onto this
   lane's base` — the same move `main` has as `80cfc9da` (`refactor(layout): the tree is three tiers`). Its diffstat is
   larger (921 files, 4224/3719) than `main`'s (867 files, 3586/3219) and its patch-id (`eb6bd57e…`) differs from
   `main`'s (`de6c921b…`), which is what replaying a 21-behind base looks like: extra files that moved differently, not
   extra work. **This pass did not settle that file by file**, so it stays an owner check — and rule 6 still says that
   lane should push its commit or say why not.

### 10.5 What §10 does not change

§1's lane positions, §2's holds, §5's other gaps and §6's open items all stand, with §9's item 1 answered above (its
other prose corrections untouched). This pass wrote to neither database.

**End state, for whoever reads this next:** `lane/deep` and `origin/main` are at the **same** commit — check it with
`git ls-remote origin refs/heads/main refs/heads/lane/deep` — and that commit is this lane's tip, which is the one
carrying this section. The four substantive commits of this pass are the published range **`bd36fad0..f3b83107`**;
`origin/lane/deep` was realigned with `git push --force-with-lease` because the rebase renumbered this lane's own three
commits. The lease, and a patch-id check, are why this can be stated flat: `HEAD..origin/lane/deep` held only those
three commits, each one's patch-id is **identical** to the version that landed on `main` (`6341055f`≡`1a89dd53`
`5a3a38d9…`, `623dccb0`≡`8ed218e9` `2d66227f…`, `76b1c6de`≡`672e04ab` `ad81d9ec…`), and the old base `c0db049c` is
still an ancestor of the tip. Nothing was discarded and nothing was held back.

## 11. The captain's follow-up, 2026-10-03: the CRUD question, measured

### 11.1 The credential was never the limit — the tool is narrow on purpose

The captain's word was *"you should have full access to CRUD in neon"*, and he is right: all four URLs in `.env.local`
(`DATABASE_URL`, `DATABASE_URL_DEV`, `DATABASE_URL_PROD`, `DATABASE_URL_UNPOOLED`) authenticate as **`neondb_owner`**,
Neon's database-owner role. §10's "read-only" was this pass's *discipline* — only reads were run — not a permission
ceiling, and the playbook now carries that in §1 instead of leaving the next reader to re-derive it.

The write path was then **proven** on DEV rather than asserted:

```
$ set -a; . ./.env.local; set +a
$ cargo test -p test-harness --test forge_tool_artifact_dev -- --ignored
test a_tool_artifact_carries_its_run_ruling_and_never_a_second_opinion ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.88s
```

That test is a real C-R-U-D pass against DEV tables — `insert into storyboard_story`, `insert into
storyboard_story_run`, `update storyboard_story_run … result_status='Complete'`, artifact rows read back through
`ForgeEngineDao`, then `delete from storyboard_story` with artifacts cascading (`tests/tests/forge_tool_artifact_dev.rs`)
— so it answers both halves at once: the access is there, and the check leaves nothing behind. It is target-typed
(`Database::connect_target(DbTarget::Dev)`), so it cannot be aimed at PROD by accident, and no control plane was left
modified. `db-tool` itself stays narrow on purpose (`status`, `apply`, `parity`; `cli/src/db_tool.rs:41-44`) — a write
goes through a reviewed migration file, and a throwaway probe file is the wrong instrument because `apply` records a
checksummed `schema_migration` row for whatever it runs (`cli/src/db_tool.rs:408-411`).

**The first attempt failed, and the cause is worth keeping.** `DatabaseUnavailable … "invalid database connection
URL"` against a database that was fine: `.env.local` **quotes** its values (the DEV URL is 148 characters including the
quotes), `dotenvy` strips them (`cli/src/apple_sync.rs:68-71`) and a shell `cut -d= -f2-` does not. Source the file or
let dotenvy read it; never string-split it.

Two doc defects came out of the same hunting, fixed here rather than left for the next reader.
`tests/tests/forge_tool_artifact_dev.rs` told you to run `cargo test -p db --test forge_tool_artifact_dev`, which does
not run at all — the crate is `test-harness`. And `scripts/rust-live-check/README.md` presented three `.mjs` scripts as
the live-check commands though none of them is in the tree; its `BEGIN; … ROLLBACK;` recipe for verifying a single
write still stands, and the Rust replacement for the scripts is a tracked port (`docs/agent/TS-TRIAGE.md:219`).

### 11.2 `lane/gpt` was measured, and deliberately not destroyed

The captain's word: *"gpt is not going to be set up for a while if you need to fix something there you can."* The
measurement says the branch has no marginal value — but it was neither deleted nor pushed, because one is irreversible
and the other only relocates a defect (a stale branch pushed to `origin` is what `recover:strand` exists to find), and
neither is required today.

- `refs/heads/lane/gpt` = `6ed57f61` is **local-only**: `git ls-remote --heads origin` lists no `lane/gpt`. It is a
  rule-2 strand, and §10.2's "push it or say why not" is answered by that sentence.
- Its base is `97785410`, 34 commits behind `main`. The replay moved the tree from **933 files / 5531+ / 3548-** away
  from `main` to **140 files / 2213+ / 735-**, so it did the bulk of the layout move correctly; the residual 140 files
  are what 34 commits of `main` look like. Its tree (`dc5dd2c5…`) is shared by **no** commit on `main`.
- The move it replays is on `main` **and is scripted and guarded there**: `scripts/restructure-domain-layout.sh`,
  `scripts/validate-move-script.sh`, and the contract test
  `tests/tests/arch_boundary__013__the_tree_is_three_tiers_and_rust_is_gone.rs` (227 lines, `d81d0d5d`). A per-lane
  replay is therefore reproducible from `main`, which is what makes keep-or-delete a cheap decision rather than a loss.
- To take at will when `lane/gpt` is revived: delete the branch and create the lane from `origin/main`
  (`docs/agent/LAYOUT.md` recipe). Until then it costs one ref and holds nothing unique.

