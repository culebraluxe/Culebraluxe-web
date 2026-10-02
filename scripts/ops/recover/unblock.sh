#!/usr/bin/env bash
set -euo pipefail

# UNBLOCK — why this window cannot commit, pull or push, and the one command that fixes it.
#
#   scripts/ops/recover/unblock.sh                every worktree on this machine, one table
#   scripts/ops/recover/unblock.sh --here         only the worktree you are standing in
#   scripts/ops/recover/unblock.sh --here --fix   repair the safe local block (detached HEAD -> a named branch)
#   scripts/ops/recover/unblock.sh --fail-over 0  exit 1 when a window is blocked (cron / gate use)
#
# WHY THIS EXISTS. "I am git blocked" is not one condition, it is five, and from a GUI they all look the same:
# a detached HEAD has no branch to push, a branch with no upstream has nowhere to push it, work that is on no
# remote is one disk failure from gone, a worktree hundreds of commits behind turns every edit into a conflict
# magnet, and a half-finished rebase refuses everything. A tool that says which one you have, and prints the exact
# command, is the difference between a person who can unblock themselves and a person who has to ask an agent.
#
# WHAT IT WILL NOT DO. No merge, rebase, reset, stash, delete or force, ever. `--fix` refuses to touch any worktree
# other than the one you are standing in, because the other lanes may be live and their dirty files may be someone's
# afternoon. Publishing a branch, landing a commit and deleting a lane are decisions with receipts
# (scripts/ops/recover/relay.sh); this script only removes the reasons git refuses to start.
#
# READ-ONLY except for `--fix` on the current worktree, and that only moves a name, never a file.

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
BASE="${UNBLOCK_BASE:-origin/main}"
HERE=0
FIX=0
FAIL_OVER=""
while [ $# -gt 0 ]; do
  case "$1" in
    --here) HERE=1; shift ;;
    --fix) FIX=1; shift ;;
    --fail-over)
      [ $# -ge 2 ] || { echo "unblock: --fail-over needs a number" >&2; exit 2; }
      FAIL_OVER="$2"; shift 2 ;;
    -h|--help) sed -n '2,21p' "$0" | sed -e 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unblock: unknown argument '$1' (try --help)" >&2; exit 2 ;;
  esac
done
cd "$ROOT"

if ! git rev-parse --verify --quiet "$BASE" >/dev/null; then
  echo "unblock: no $BASE to measure against. Fetch, or set UNBLOCK_BASE." >&2
  exit 2
fi

MAIN_WT="$(git worktree list --porcelain 2>/dev/null | sed -n '1s/^worktree //p')"
lane_of() { # $1 = worktree path -> a branch name that reads like where it came from
  if [ "${1%/}" = "${MAIN_WT%/}" ]; then printf 'trunk'; else printf 'lane/%s' "$(basename "${1%/}" | sed -e 's/^Culebraluxe-web-\{0,1\}//')"; fi
}
short_path() { # $1 = path, $HOME shown as ~, clipped so a row stays one line
  local p="$1"
  # shellcheck disable=SC2088  # the tilde here IS the display text, not a path to expand
  case "$p" in "$HOME"/*) printf -v p '~/%s' "${p#"$HOME"/}" ;; esac
  if [ "${#p}" -gt 58 ]; then printf '%s...' "${p:0:55}"; else printf '%s' "$p"; fi
}

blocked=0
atrisk=0
print_one() { # $1 = worktree path
  local wt="$1" lane branch sha up lock ops behind ahead unpub dirty verdict fixcmd fixsrc
  lane="$(lane_of "$wt")"
  branch="$(git -C "$wt" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
  if [ "$branch" = "HEAD" ]; then branch="(detached HEAD)"; fi
  sha="$(git -C "$wt" rev-parse --short HEAD 2>/dev/null || echo '?')"
  up="$(git -C "$wt" rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
  behind=""; ahead=""
  if read -r b a < <(git -C "$wt" rev-list --left-right --count "$BASE...HEAD" 2>/dev/null); then
    behind="$b"; ahead="$a"
  fi
  # On NO remote at all: the count that decides whether this is recoverable or a story.
  unpub="$(git -C "$wt" rev-list --count HEAD --not --remotes 2>/dev/null || echo '?')"
  dirty="$(git --no-optional-locks -C "$wt" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
  lock=0
  if ( cd "$wt" && [ -e "$(git rev-parse --git-path index.lock)" ] ); then lock=1; fi
  ops=""
  for op in rebase-merge rebase-apply MERGE_HEAD CHERRY_PICK_HEAD; do
    if ( cd "$wt" && [ -e "$(git rev-parse --git-path "$op")" ] ); then ops="${ops:+$ops, }$op"; fi
  done

  if [ -n "$ops" ]; then
    verdict="HARD BLOCK: $ops in progress — every git command will refuse"
    if [ "$ops" = "MERGE_HEAD" ]; then
      fixcmd="finish the merge (git -C $wt commit) or undo it (git -C $wt merge --abort) — abort DISCARDS the resolution work, so look at git status first"
    else
      fixcmd="finish it, or: git -C $wt rebase --abort"
    fi
    blocked=$((blocked + 1))
  elif [ "$lock" = "1" ]; then
    verdict="HARD BLOCK: index.lock present"
    fixcmd="if no git is running here: rm '$(cd "$wt" && git rev-parse --git-path index.lock)'"
    blocked=$((blocked + 1))
  elif [ "${unpub:-0}" != "0" ] && [ "${unpub:-0}" != "?" ]; then
    verdict="UNPUBLISHED: $unpub commit(s) exist on no remote at all"
    # The source of a push is the branch if there is one, and HEAD if there is not: `git push origin lane/x` would
    # look for a local branch called lane/x and fail with "src refspec does not match any".
    if [ "$branch" = "(detached HEAD)" ]; then fixsrc="HEAD:$lane"; else fixsrc="$branch"; fi
    fixcmd="git -C $wt push -u origin $fixsrc   # a short-lived branch is allowed now; land it after (relay.sh)"
    blocked=$((blocked + 1)); atrisk=$((atrisk + unpub))
  elif [ "$branch" = "(detached HEAD)" ]; then
    verdict="DETACHED HEAD: no branch, so a GUI has nothing to push"
    fixcmd="git -C $wt switch -c $lane"
    blocked=$((blocked + 1))
  elif [ -z "$up" ]; then
    verdict="NO UPSTREAM: the branch exists on this disk only"
    fixcmd="git -C $wt push -u origin $branch"
    blocked=$((blocked + 1))
  elif [ "${behind:-0}" != "0" ] && [ "${dirty:-0}" != "0" ]; then
    verdict="STALE + DIRTY: $behind behind with $dirty uncommitted — a conflict magnet, not yet a refusal"
    fixcmd="keep the edits: git -C $wt stash -u && git -C $wt merge --ff-only $BASE && git -C $wt stash pop"
    blocked=$((blocked + 1))
  else
    verdict="usable"
    fixcmd=""
  fi

  printf '%s  %s  %s\n' "$(short_path "$wt")" "$sha" "$branch"
  printf '    position   %s behind / %s ahead of %s        upstream  %s\n' "${behind:-?}" "${ahead:-?}" "$BASE" "${up:-none}"
  printf '    working    %s uncommitted            commits here on no remote: %s\n' "$dirty" "${unpub:-?}"
  printf '    verdict    %s\n' "$verdict"
  if [ -n "$fixcmd" ]; then printf '    fix        %s\n' "$fixcmd"; fi
  return 0
}

here="$(git rev-parse --show-toplevel 2>/dev/null || echo "$ROOT")"
printf 'unblock  base=%s@%s  %s\n' "$BASE" "$(git rev-parse --short "$BASE")" "$(date -u '+%Y-%m-%d %H:%M')Z"

if [ "$HERE" = "1" ]; then
  print_one "$here"
  echo
  printf 'unblock: --here reported 1 worktree (%s). Run without --here for every lane.\n' "$(short_path "$here")"
else
  while read -r wt; do print_one "$wt"; echo; done < <(git worktree list --porcelain 2>/dev/null | awk '/^worktree /{print $2}')
  printf 'unblock: %s worktree(s) blocked, %s commit(s) recoverable only from this disk\n' "$blocked" "$atrisk"
  if [ "$atrisk" -gt 0 ]; then
    printf '%s\n' 'ATTENTION: unpublished work above is one disk failure from gone. Publish or land it (relay.sh); do not delete the worktree.'
  fi
fi

if [ "$FIX" = "1" ]; then
  [ "$HERE" = "1" ] || { echo "unblock: --fix only repairs the worktree you are standing in; add --here." >&2; exit 2; }
  branch="$(git -C "$here" rev-parse --abbrev-ref HEAD)"
  unpub="$(git -C "$here" rev-list --count HEAD --not --remotes 2>/dev/null || echo '?')"
  if [ "$branch" != "HEAD" ]; then
    echo "unblock: --fix did nothing — this worktree is on branch '$branch', which is a branch git accepts."
  elif [ "${unpub:-0}" != "0" ]; then
    echo "unblock: --fix refuses — HEAD holds $unpub commit(s) on no remote. Publish them first, so a branch name and not just HEAD is holding them: git -C $here push -u origin $(lane_of "$here")" >&2
    exit 1
  else
    lane="$(lane_of "$here")"
    git -C "$here" switch -c "$lane"
    echo "unblock: now on branch '$lane' at $(git -C "$here" rev-parse --short HEAD). A GUI can commit, pull and push again; land it on main with: git push origin HEAD:main"
  fi
fi

if [ -n "$FAIL_OVER" ] && [ "$blocked" -gt "$FAIL_OVER" ]; then
  printf 'unblock: %s blocked (over the --fail-over %s limit)\n' "$blocked" "$FAIL_OVER" >&2
  exit 1
fi
exit 0
