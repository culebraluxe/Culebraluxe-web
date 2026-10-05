# Phase 01: Lane-Muse Git Tree Fix and Park DeepSeek Leftovers

This phase fixes the good git tree you asked for. Your lane-muse worktree was copied from lane-deep and still reports `lane/deep` branch, which means it trails Deep's commits instead of origin/main. It parks the 4 DeepSeek TST-WF-DECISION triage docs that came with the copy so Auto Run stays quiet, creates a fresh `lane/muse` branch on top of origin/main, and proves the worktree is healthy with a cargo check stub. By the end you have a valid fourth lane that appears in `git worktree list` correctly and nothing extra tries to run.

## Tasks

- [ ] Park the copied DeepSeek triage docs and capture pre-fix state:
  - Run `ls -la /Users/Shared/dev/src/lane-muse/.maestro/playbooks/Initiation/` and save to `Working/Phase-01-before.txt`
  - If `docs/triage/` exists with TST files, list it: `ls -la docs/triage/ 2>&1 | head -20` and archive any `TST-WF-DECISION*.md` with `mkdir -p Working/Archive && mv docs/triage/TST-* Working/Archive/ 2>&1 || true`
  - Run `git branch --show-current; git status --porcelain; git log --oneline -5; git worktree list` and save to `Working/Phase-01-git-pre.txt`
  - Ensure Working/ dir exists: `mkdir -p /Users/Shared/dev/src/lane-muse/.maestro/playbooks/Initiation/Working`

- [ ] Fix git worktree branch from lane/deep to lane/muse on origin/main:
  - Run `cd /Users/Shared/dev/src/lane-muse && git fetch origin`
  - Check if `lane/muse` branch exists: `git branch --list "lane/muse"` and remote `git branch -r --list "origin/lane/muse"`
  - If local `lane/muse` exists and is not on origin/main, reset it: `git checkout lane/muse && git reset --hard origin/main`
  - If local `lane/muse` does NOT exist, create it fresh: `git checkout -b lane/muse origin/main`
  - If currently still on `lane/deep`, switch: `git checkout lane/muse` after creation
  - Verify: `git branch --show-current` must output `lane/muse`, `git log --oneline -1` must match `git log origin/main --oneline -1`, and `git worktree list | grep lane-muse` must show `[lane/muse]`
  - Save verification to `Working/Phase-01-git-post.txt` including `git log --oneline -3` and `git worktree list`

- [ ] Ensure lane-muse working directory is clean for Muse work:
  - Run `cd /Users/Shared/dev/src/lane-muse && git status --porcelain` and ensure only expected .maestro files or Working/ untracked are present
  - If `opencode.json` or `docs/agent/LAYOUT.md` show modifications, keep them staged for next phases but ensure no conflicting rebase state: `git diff --stat`
  - Run `pnpm install --frozen-lockfile 2>&1 | tail -20` if node_modules missing, otherwise `ls -la node_modules/.bin/opencode 2>&1 | head`
  - Write clean-state summary to `Working/Phase-01-clean.txt`

- [ ] Verify Phase 1 prototype works end-to-end:
  - Confirm `git -C /Users/Shared/dev/src/lane-muse branch --show-current` equals `lane/muse`
  - Confirm `git -C /Users/Shared/dev/src/lane-muse log --oneline -1` equals `origin/main` head
  - Confirm `/Users/Shared/dev/src/lane-muse/.maestro/playbooks/Initiation/` contains exactly 4 Phase files and no TST leftover beyond what was overwritten (list file count)
  - Run `cd /Users/Shared/dev/src/lane-muse && cargo check -p workflow --all-targets 2>&1 | tail -20` and save to `Working/Phase-01-cargo-check.txt` — must exit 0
  - Print final one-liner: branch, base sha, worktree list line, cargo check status
