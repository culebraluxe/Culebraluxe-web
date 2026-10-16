#!/usr/bin/env bash
set -euo pipefail

# UNBLOCK — why this window cannot commit, pull or push, and the one command that fixes it.
#
#   scripts/ops/recover/unblock.sh                every worktree on this machine, one table
#   scripts/ops/recover/unblock.sh --here         only the worktree you are standing in
#   scripts/ops/recover/unblock.sh --here --fix   detached HEAD -> a named branch (in a jj workspace: refuses, prints the jj form)
#   scripts/ops/recover/unblock.sh --fail-over 0  exit 1 when a window is blocked (cron / gate use)
#
# WHY THIS EXISTS. "I am git blocked" is not one condition, it is five, and from a GUI they all look the same:
# a detached HEAD has no branch to push, a branch with no upstream has nowhere to push it, work that is on no
# remote is one disk failure from gone, a worktree hundreds of commits behind turns every edit into a conflict
# magnet, and a half-finished rebase refuses everything. A tool that says which one you have, and prints the exact
# command, is the difference between a person who can unblock themselves and a person who has to ask an agent.
#
# UNDER jj (docs/agent/JJ-INTERFACE.md). A jj workspace — the control plane and every lane converted on 2026-10-15 —
# IS detached HEAD in git's eyes, because `git branch --show-current` is empty there (jj keeps its working copy as a
# change, not a branch), so four of those five conditions cannot happen to it at all. Measured 2026-10-16: 24 worktree
# records, the control plane plus a throwaway lane among them, and this tool called every jj workspace DETACHED HEAD
# and offered `git switch -c` — a git write into a tree jj owns, the exact mixing House Rule 2 forbids. So a jj row
# now reads jj's own measure instead of git's: `main@origin..@ & ~empty()` counts the changes above the trunk that no
# remote has, which is the honest form of "recoverable only from this disk" — `git rev-list HEAD --not --remotes`
# answers 0 for the same work, because jj's working-copy commit is reachable from no git ref. `--fix` refuses in a jj
# workspace and prints the jj command instead of a branch name. Git's numbers in a jj row — the sha, behind/ahead, and
# "uncommitted" — are git's view of jj's last export to refs, which lags the working copy, so the row labels them and
# prints jj's count as the one to trust (measured 2026-10-16: git called a lane "1 ahead, 2 uncommitted" while jj found
# nothing above the trunk at all).
#
# WHAT IT WILL NOT DO. No merge, rebase, reset, stash, delete or force, ever. `--fix` refuses to touch any worktree
# other than the one you are standing in, because the other lanes may be live and their dirty files may be someone's
# afternoon. Publishing a branch (a jj lane publishes a bookmark), landing a commit and deleting a lane are decisions
# with receipts (scripts/ops/recover/relay.sh); this script only removes the reasons git refuses to start.
#
# READ-ONLY except for `--fix` on the current worktree, and that only moves a name, never a file. In a jj workspace it
# reads jj with --ignore-working-copy: no snapshot, no bookmark moved, no git ref written — so a window mid-edit is
# reported from its last snapshot, and the row says so whenever git and jj disagree about what is above the trunk.

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
    -h|--help) sed -n '2,37p' "$0" | sed -e 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unblock: unknown argument '$1' (try --help)" >&2; exit 2 ;;
  esac
done
# The directory the human was standing in when they ran this, captured before the cd below, because --here needs it and
# the cd erases it. Measured 2026-10-16: resolving --here after the cd reported the control plane from every lane, git
# lanes included, and --fix would have named a branch in the wrong tree.
CALLER_PWD="$PWD"
cd "$ROOT"

if ! git rev-parse --verify --quiet "$BASE" >/dev/null; then
  echo "unblock: no $BASE to measure against. Fetch, or set UNBLOCK_BASE." >&2
  exit 2
fi

MAIN_WT="$(git worktree list --porcelain 2>/dev/null | sed -n '1s/^worktree //p')"
lane_of() { # $1 = worktree path -> the branch/bookmark name that reads like where it came from
  if [ "${1%/}" = "${MAIN_WT%/}" ]; then printf 'trunk'; else printf 'lane/%s' "$(basename "${1%/}" | sed -e 's/^Culebraluxe-web-\{0,1\}//' -e 's/^lane-\{1,\}//')"; fi
}
is_jj() { [ -e "${1%/}/.jj" ]; }   # a jj workspace, colocated or not: .jj is a directory here, and a file in a plain checkout
publish_name() { # $1 = worktree path -> the bookmark a jj workspace publishes: main in the control plane, lane/<name> in a lane
  # The control plane is not a lane and must never be handed a lane's recipe: its unpublished work lands on main
  # (docs/agent/JJ-INTERFACE.md rule 5), where a lane publishes a throwaway bookmark called after the directory.
  if [ "${1%/}" = "${MAIN_WT%/}" ]; then printf 'main'; else lane_of "$1"; fi
}
unpublished_jj() { # $1 = jj workspace path -> non-empty changes above main@origin that no remote has
  # jj's own answer, and the only one that sees a working copy jj has not exported to a git ref. `.` would print
  # one character per change; the count is the answer. `~` is jj's set difference (measured 2026-10-16, jj 0.46).
  local n
  n="$(jj -R "${1%/}" --ignore-working-copy log --no-graph -r 'main@origin..@ & ~empty()' -T '"."' 2>/dev/null | wc -c | tr -d ' ')"
  # --ignore-working-copy on purpose: this script reports, it must not snapshot a window mid-edit. So the count is
  # "what the last snapshot saw", which is what a table wants; a worker's newest keystroke is not in it yet.
  [ -n "$n" ] || n='?'
  printf '%s' "$n"
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
  local wt="$1" lane branch sha up lock ops behind ahead unpub dirty verdict fixcmd fixsrc isjj jjn bm
  lane="$(lane_of "$wt")"
  branch="$(git -C "$wt" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')"
  if [ "$branch" = "HEAD" ]; then branch="(detached HEAD)"; fi
  # A jj workspace IS "(detached HEAD)" to git and is not blocked by it at all. Detect it once, here, and let both the
  # verdict chain and the row read jj's own numbers.
  isjj=0; jjn=""
  if is_jj "$wt"; then isjj=1; jjn="$(unpublished_jj "$wt")"; fi
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
  elif [ "$isjj" = "1" ] && [ "${jjn:-0}" != "0" ] && [ "${jjn:-0}" != "?" ]; then
    # Published the jj way: a lane bookmark, not a branch. `jj bookmark set` creates it or moves it, and
    # `jj push --bookmark` tracks the remote bookmark on the first push — measured 2026-10-16, no --allow-new needed.
    bm="$(publish_name "$wt")"
    if [ "$bm" = "main" ]; then
      verdict="UNPUBLISHED: $jjn change(s) above $BASE are on no remote (jj's count; git's is $unpub) — this is the control plane"
      fixcmd="jj -R $wt bookmark set main -r @  &&  jj -R $wt push --bookmark main   # land it on main once the gates pass"
    else
      verdict="UNPUBLISHED: $jjn change(s) above $BASE are on no remote (jj's count; git's is $unpub)"
      fixcmd="jj -R $wt bookmark set $bm -r @  &&  jj -R $wt push --bookmark $bm"
    fi
    blocked=$((blocked + 1)); atrisk=$((atrisk + jjn))
  elif [ "$isjj" != "1" ] && [ "${unpub:-0}" != "0" ] && [ "${unpub:-0}" != "?" ]; then
    # Not reached by a jj workspace, on purpose: git counts commits, and `jj commit` creates an empty commit whenever
    # the working copy matches its parent — a lane that did nothing can show "1 commit on no remote" to git while
    # holding no change at all. Measured 2026-10-16 in a throwaway lane. jj's `~empty()` is the honest count, and the
    # jj cases above own every jj row.
    verdict="UNPUBLISHED: $unpub commit(s) exist on no remote at all"
    # The source of a push is the branch if there is one, and HEAD if there is not: `git push origin lane/x` would
    # look for a local branch called lane/x and fail with "src refspec does not match any".
    if [ "$branch" = "(detached HEAD)" ]; then fixsrc="HEAD:$lane"; else fixsrc="$branch"; fi
    fixcmd="git -C $wt push -u origin $fixsrc   # a short-lived branch is allowed now; land it after (relay.sh)"
    blocked=$((blocked + 1)); atrisk=$((atrisk + unpub))
  elif [ "$isjj" = "1" ]; then
    # Nothing above the trunk that no remote has: under jj there is no detached HEAD to name and no upstream to be
    # missing — the working copy is a change of its own. A stale base is worth saying, but it is not a block: jj
    # rebases a change with a dirty window in it, which is the whole difference from the git row below (`git stash`,
    # which this repository forbids, is what that row would need).
    if [ "${jjn:-?}" = "?" ]; then
      verdict="UNKNOWN: jj could not answer in this workspace — look: jj -R $wt status"
    elif [ "${behind:-0}" != "0" ]; then
      verdict="STALE: $behind behind $BASE — nothing unpublished, but the base is old"
      fixcmd="jj -R $wt git fetch  &&  jj -R $wt rebase -d $BASE"
    else
      verdict="usable (jj workspace; nothing above $BASE is non-empty)"
      if [ "${unpub:-0}" != "0" ]; then
        # git is ahead of jj here, and the reason is worth one clause: this tool reads jj WITHOUT snapshotting, so a
        # window mid-edit can be invisible to jj while git still counts the last exit. Say so, and say the one command
        # that settles it — an honest lag beats a silent one.
        verdict="$verdict — git counts $unpub above it that jj cannot see, because this tool reads jj without snapshotting (jj -R $wt status settles it)"
      fi
    fi
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

  if [ "$isjj" = "1" ]; then
    printf '%s  %s  (jj workspace)\n' "$(short_path "$wt")" "$sha"
    printf '    position   %s behind / %s ahead of %s        upstream  n/a (jj)\n' "${behind:-?}" "${ahead:-?}" "$BASE"
    printf '    working    %s change(s) above %s on no remote (jj count; %s path(s) git sees as uncommitted)\n' "${jjn:-?}" "$BASE" "${dirty:-?}"
  else
    printf '%s  %s  %s\n' "$(short_path "$wt")" "$sha" "$branch"
    printf '    position   %s behind / %s ahead of %s        upstream  %s\n' "${behind:-?}" "${ahead:-?}" "$BASE" "${up:-none}"
    printf '    working    %s uncommitted            commits here on no remote: %s\n' "$dirty" "${unpub:-?}"
  fi
  printf '    verdict    %s\n' "$verdict"
  if [ -n "$fixcmd" ]; then printf '    fix        %s\n' "$fixcmd"; fi
  return 0
}

here="$(git -C "$CALLER_PWD" rev-parse --show-toplevel 2>/dev/null || echo "$ROOT")"
printf 'unblock  base=%s@%s  %s\n' "$BASE" "$(git rev-parse --short "$BASE")" "$(date -u '+%Y-%m-%d %H:%M')Z"

if [ "$HERE" = "1" ]; then
  print_one "$here"
  echo
  printf 'unblock: --here reported 1 worktree (%s). Run without --here for every lane.\n' "$(short_path "$here")"
else
  while read -r wt; do print_one "$wt"; echo; done < <(git worktree list --porcelain 2>/dev/null | awk '/^worktree /{print $2}')
  printf 'unblock: %s worktree(s) blocked, %s unit(s) of work recoverable only from this disk\n' "$blocked" "$atrisk"
  if [ "$atrisk" -gt 0 ]; then
    printf '%s\n' 'ATTENTION: unpublished work above is one disk failure from gone. Publish or land it (relay.sh); do not delete the worktree.'
  fi
fi

if [ "$FIX" = "1" ]; then
  [ "$HERE" = "1" ] || { echo "unblock: --fix only repairs the worktree you are standing in; add --here." >&2; exit 2; }
  if is_jj "$here"; then
    # A jj workspace has no detached HEAD to repair. `git switch -c` here would be a git write into a tree jj owns —
    # the mixing House Rule 2 forbids — so --fix reports instead: refuse if there is unpublished work, and otherwise
    # say plainly that there was nothing to fix rather than inventing a branch.
    jjn="$(unpublished_jj "$here")"
    bm="$(publish_name "$here")"
    if [ "${jjn:-0}" != "0" ] && [ "${jjn:-0}" != "?" ]; then
      if [ "$bm" = "main" ]; then
        {
          echo "unblock: --fix refuses — $jjn change(s) above $BASE are on no remote, and this is the control plane."
          echo "  land them on main: jj -R $here bookmark set main -r @  &&  jj -R $here push --bookmark main"
          echo "  (or publish a throwaway bookmark to park them; docs/agent/JJ-INTERFACE.md)"
        } >&2
      else
        {
          echo "unblock: --fix refuses — $jjn change(s) above $BASE are on no remote. Publish them first, as a lane bookmark:"
          echo "  jj -R $here bookmark set $bm -r @  &&  jj -R $here push --bookmark $bm"
          echo "  (or land them on main; docs/agent/JJ-INTERFACE.md)"
        } >&2
      fi
      exit 1
    fi
    echo "unblock: --fix did nothing — a jj workspace is never blocked by a missing branch: the work is a change, not a HEAD."
    echo "unblock: sync with: jj -R $here git fetch && jj -R $here rebase -d $BASE"
    exit 0
  fi
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
