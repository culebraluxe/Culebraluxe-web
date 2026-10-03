#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/wip-snapshot.sh — snapshot every dirty worktree into refs/wip/<name>.
#
# WHY THIS EXISTS
#   A worker's window can close mid-edit: a quota ceiling, a context limit, or
#   the 5-hour model lockout that hit on 2026-10-01 while a commit was still
#   only in the working tree. When the only copy of the work is the working
#   tree, a closed window costs code. This job runs on a clock and writes a
#   LOCAL commit of whatever is dirty, so a closed window can never cost code.
#
# WHAT IT DOES
#   For each worktree in `git worktree list`: stage the working tree into a
#   TEMPORARY index, `write-tree` + `commit-tree` it, and point
#   refs/wip/<worktree-name> at the result. Tracked edits, untracked new files
#   and deletions are all included — an agent that dies mid-file is captured.
#
# WHAT IT DOES NOT DO
#   It never touches the working tree, the real index, HEAD, a branch, a
#   remote or a hook. It cannot rebase, cannot push and cannot lock anyone out
#   of anything. A worker mid-edit cannot tell it ran. Refs live in
#   refs/wip/* — never pushed, never merged, never seen by CI.
#
# RECOVER
#   git -C <worktree> diff  HEAD refs/wip/<name>        # what was dirty
#   git -C <worktree> show  refs/wip/<name> --stat      # what was captured
#   git -C <worktree> checkout refs/wip/<name> -- <path>
#
# RUN
#   scripts/wip-snapshot.sh     # all worktrees, one line each
#   pnpm wip:now                # same thing
#   Installed as com.culebraluxe.wip-snapshot (every 300s) by `pnpm wip:install`.
# ---------------------------------------------------------------------------
set -u

ROOT="${CULEBRALUXE_REPO:-$(cd "$(dirname "$0")/.." && pwd)}"
if [ ! -d "$ROOT/.git" ] && [ ! -f "$ROOT/.git" ]; then
  echo "wip-snapshot: not a git worktree: $ROOT" >&2
  exit 1
fi

# Build output can never be worth snapshotting, and one stray node_modules would
# turn a 5-minute job into a 5-GB one. .gitignore already covers these on this
# repo; the extra ignore file makes it true on any repo this script is copied to.
#
# It has to be an ignore FILE, not a `:(exclude)` pathspec: naming an ignored
# path on the command line makes `git add` exit 1 with "The following paths are
# ignored by one of your .gitignore files", which reports a failure that never
# happened. (It also means a worktree with no node_modules "passes" while one
# with node_modules "fails" — the same non-event, decided by what is on disk.)
IGNORES="$(mktemp -t cul-wip-ignores)"
trap 'rm -f "$IGNORES"' EXIT
cat > "$IGNORES" <<'IGNORE_EOF'
node_modules
**/node_modules
**/target
**/dist
**/build
.next
**/.next
**/.turbo
.pnpm-store
**/.pnpm-store
.vercel
**/.vercel
__pycache__
**/__pycache__
IGNORE_EOF

worktrees="$(git -C "$ROOT" worktree list --porcelain | awk '/^worktree /{sub(/^worktree /, ""); print}')"

while IFS= read -r wt; do
  [ -n "$wt" ] || continue
  if [ ! -d "$wt" ]; then
    echo "missing   $(basename "$wt")  (worktree gone from disk; skipped)"
    continue
  fi
  name="$(basename "$wt")"

  idx="$(mktemp -t cul-wip-index)"
  rm -f "$idx"   # git wants to create the index itself; an empty file is not a valid index

  if ! GIT_INDEX_FILE="$idx" git -C "$wt" -c core.excludesFile="$IGNORES" add -A -- . >/dev/null 2>&1; then
    # Nothing was changed anywhere: a locked index, a read-only file, an I/O error.
    # Report and move on — the next tick retries, and no state was mutated.
    echo "failed    $name  (could not stage; working tree untouched, next run retries)"
    rm -f "$idx"
    continue
  fi

  tree="$(GIT_INDEX_FILE="$idx" git -C "$wt" write-tree 2>/dev/null || true)"
  head_tree="$(git -C "$wt" rev-parse 'HEAD^{tree}' 2>/dev/null || true)"
  rm -f "$idx"

  if [ -z "$tree" ]; then
    echo "failed    $name  (write-tree produced nothing)"
    continue
  fi
  if [ "$tree" = "$head_tree" ]; then
    echo "clean     $name  (HEAD already equals the working tree)"
    continue
  fi

  stamp="$(date '+%Y-%m-%d %H:%M:%S')"
  commit="$(git -C "$wt" commit-tree "$tree" -p HEAD -m "wip($name): snapshot $stamp" 2>/dev/null || true)"
  if [ -z "$commit" ]; then
    echo "failed    $name  (commit-tree refused; tree object is still in the object store)"
    continue
  fi
  if git -C "$wt" update-ref "refs/wip/$name" "$commit" 2>/dev/null; then
    echo "snapshot  $name -> refs/wip/$name ${commit:0:8}  ($stamp)"
  else
    echo "failed    $name  (update-ref refs/wip/$name refused)"
  fi
done <<< "$worktrees"
