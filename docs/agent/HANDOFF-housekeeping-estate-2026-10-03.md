# Handoff — the estate after a housekeeping pass (2026-10-03)

Session of 2026-10-03, agent in `src/lane-deep`. Housekeeping only: **no product file was changed**, no product
compile or test ran, and no other lane's *work* was written (the other worktrees were read with
`--no-optional-locks`). One exception, named here and in §8: an untracked `.DS_Store` inside `lane-claude` was deleted
as Finder litter on the widened instruction — no tracked file, and nothing under version control, was touched there.
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
| S12 | `refs/archive/lane-deep-replay` → `692d99c7`, which is also `lane/claude`'s tip | `git rev-parse refs/archive/lane-deep-replay` |
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
| the third commit for this file (§8, the trap note, and the rows §8 falsified) — `git log -1 --format=%h -- docs/agent/HANDOFF-housekeeping-estate-2026-10-03.md` | §8's junk pass with its receipts; S2/S3/S4/S11 and H4 brought current or marked resolved; the `--exclude`-order trap | `git diff --check` → clean; `find … -name .DS_Store` → 0; `git fsck --no-dangling` → exit 0 with no output; `git for-each-ref \| wc -l` → 4230 (was 4232). Docs-only, so no compile is owed; same relay push form |

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
2. `lane/claude` (`692d99c7`) and `lane/gpt` (`6ed57f61`): **off limits to this lane** (captain, 2026-10-03). Their owners push them or nobody does — this pass did not. Both trees already exist (`/Users/Shared/dev/src/lane-claude`, `/Users/Shared/dev/src/lane-gpt`), so no `git worktree add` is owed and one would only fail with *already registered*; from inside the lane the whole job is `git push -u origin lane/<name>`.
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
- **`refs/archive/lane-deep-replay`** → `692d99c7`: kept on purpose, because §6.2 tells Claude's owner to `rebase
  lane/claude`, after which this ref is the only thing still holding `693d…`'s replay commit (S12);
- **the 181 unreachable commits and 231 unreachable blobs** stay in the pack. `gc --prune=now` is the only thing that
  would remove the stale 6.8 MB wasm blob (S5), and it cannot be aimed — it would take the authored commits in that set
  with it. 6.8 MB is cheaper than a blind decision. (These two refs are why S4 moved from 229/177 to 231/181.)
- **`/Users/Shared/dev/build/archive-2026-10-03-from-documents/` is not junk and must not be deleted.**
  `Culebraluxe-web-claude/.env.local` (8 761 bytes, 2026-09-26) is the **only** copy of the Claude lane's environment —
  `[ -f /Users/Shared/dev/src/lane-claude/.env.local ]` → false. Whoever stands that lane up should copy it in
  (untracked), not regenerate it.
- **the four `com.culebraluxe.*.plist` copies in `build/logs`** stay: nothing references them (`grep -rn 'build/logs'
  scripts/` → the wip log dir only) and all four differ (`md5 -q`) from the installed `~/Library/LaunchAgents/…` — an
  older config backup, not litter, and launchd's file, not this lane's.

Effect: `git for-each-ref | wc -l` → 4230 (was 4232), `find … -name .DS_Store` → 0, `git fsck --no-dangling` → exit 0 with
no output, no empty ref directory left behind. Deletion receipts, for reversal: `refs/wip/ocwt` was `242fe8ee`
(blob `f15466f9`, 76 bytes, content above); `refs/remotes/codex/b4c2230` was `b4c223027cc6a1e7b086a25fa4c480e5ffeda4bf`
(landed as `b5389b76`).
