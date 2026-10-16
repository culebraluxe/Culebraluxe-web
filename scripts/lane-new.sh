#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/lane-new.sh — create a lane: one code-only jj workspace, on its own bookmark.
#
# WHY THIS EXISTS
#   Eleven lanes were made by hand on 2026-10-03 and the recipe has four steps
#   that are easy to get subtly wrong, each with a cost that only shows up later:
#   a copy of `.env.local` instead of a symlink (four rotation points for one set
#   of secrets, in lanes nobody looks at), an upstream left pointing at
#   `origin/main` (a bare `git push` then aims at the trunk), modes left at 755
#   inside a 700 tree, and `pnpm install` run "to be safe" (a 931 MB lane that
#   needs no website). This script is that recipe with the four traps closed.
#   The upstream trap is now closed by jj instead: a bookmark it creates is untracked.
#
# WHAT A LANE IS
#   Code only. ~84 MB, no node_modules, plus a build directory of its own outside the
#   tree (/Users/Shared/dev/build/rust-lane-<name>, from the lane-local
#   .cargo/config.toml this script writes): cargo holds an exclusive lock on a target
#   directory for the length of a build, so one shared directory queues every lane on
#   the machine behind whichever lane happens to be compiling. `pnpm install` is a
#   per-lane decision, not part of setup — a lane without node_modules still compiles,
#   tests, commits and lands.
#
# WHAT IT DOES
#   1. jj workspace add <sibling>/lane-<name> --name lane-<name> -r main@origin
#      then jj bookmark create lane/<name> -r main@origin
#   2. ln -sfn the two shared env files into the lane root
#   3. write the lane's own cargo config (scripts/lane-cargo-config.sh owns that rule
#      and its text; this recipe only calls it)
#   4. chmod -R go-rwx the lane (umask 077 is set first, so new files start closed)
#
#   Why a jj workspace and not a git branch: on 2026-10-15 the interface became jj
#   (docs/agent/JJ-INTERFACE.md) and `jj` cannot run in a plain git worktree at all —
#   measured that day, `jj status` inside a lane answered `There is no jj repo in "."`.
#   Measured on a throwaway workspace: `jj workspace add` inside this colocated repo ALSO
#   registers a git worktree, so the ten worktree-aware scripts keep seeing the lane, and
#   it runs no git hook — `.githooks/post-checkout` never fires for a workspace, which is
#   why step 3 is called here instead of trusted to it.
#
#   The old step 4, `git branch --unset-upstream`, is gone because the trap it closed
#   does not exist under jj: a bookmark jj creates is UNTRACKED, so a bare `jj git push`
#   cannot aim at the trunk the way a fresh git branch with origin/main as its upstream
#   could. Publish a lane by name — `jj push --bookmark lane/<name>` — and land by moving
#   main (docs/agent/JJ-INTERFACE.md, "Landing on main under jj").
#
# WHAT IT DOES NOT DO
#   No `pnpm install` (pass --with-website for that), no build, no commit, no push,
#   no roster edit: a new lane is the Captain's call, so add its row to
#   docs/agent/LAYOUT.md yourself and land that in the normal way.
#
# RUN
#   bash scripts/lane-new.sh nemotron            # create ../lane-nemotron
#   bash scripts/lane-new.sh mistral --with-website
#   bash scripts/lane-new.sh mistral --dry-run   # print, change nothing
#   bash scripts/lane-new.sh --setup-only <dir>  # internal: re-apply steps 2-4 to a
#                                               # workspace that already exists, which is
#                                               # what scripts/lane-adopt-jj.sh needs after
#                                               # it recreates a lane through jj workspace add
# ---------------------------------------------------------------------------
set -u

# scripts/lane-cargo-config.sh lives beside this script, and this script must be callable from any working
# directory, so the path is derived rather than assumed.
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

# The two shared env files are named once, here, because both the create path and --setup-only need them; a
# second copy of these paths is a second place for that rule to be wrong.
env_local="/Users/Shared/dev/.env.local"
env_scheduler="/Users/Shared/dev/.env.scheduler"

# Every step is printed before it runs, so --dry-run needs no second description of the recipe.
run() {
  echo "  + $*"
  [ "$dry_run" -eq 1 ] || "$@"
}

usage() {
  cat <<'USAGE'
usage: scripts/lane-new.sh <name> [--with-website] [--dry-run]
       scripts/lane-new.sh --setup-only <dir>

  <name>            lowercase letters, digits and dashes: lane-<name> is created
                    beside the main checkout, as a jj workspace on bookmark lane/<name>
  --with-website    also run `pnpm install --frozen-lockfile` in the new lane
                    (~30 MB real, ~847 MB as du counts it) for tailwind / pnpm build
  --dry-run         print the commands and exit without changing anything
  --setup-only <dir>  no new lane: apply only the shared env symlinks, the lane's cargo
                    config and the mode fix, to a directory that is already a workspace
                    (scripts/lane-adopt-jj.sh calls this after `jj workspace add`)
USAGE
}

name=""
with_website=0
dry_run=0
setup_only=0
setup_path=""
want_path=0
for arg in "$@"; do
  case "$arg" in
    --with-website) with_website=1 ;;
    --dry-run)      dry_run=1 ;;
    --setup-only)   setup_only=1; want_path=1 ;;
    -h|--help)      usage; exit 0 ;;
    -*)             echo "lane-new: unknown option: $arg" >&2; usage >&2; exit 2 ;;
    *)              if [ "$want_path" -eq 1 ]; then setup_path="$arg"; want_path=0; else name="$arg"; fi ;;
  esac
done

# --setup-only: the directory already exists because a workspace was just created there, so the recipe is only
# the lane-side setup — the part a git hook would have done for a git worktree and does not for a jj workspace.
# No fetch, no workspace add, no bookmark, no branch check: none of those are this mode's business.
if [ "$setup_only" -eq 1 ]; then
  if [ "$want_path" -eq 1 ] || [ -z "$setup_path" ]; then
    echo "lane-new: --setup-only needs a directory" >&2
    usage >&2
    exit 2
  fi
  [ -d "$setup_path" ] || { echo "lane-new: --setup-only: no such directory: $setup_path" >&2; exit 1; }
  for f in "$env_local" "$env_scheduler"; do
    [ -f "$f" ] || { echo "lane-new: missing shared env file: $f" >&2; exit 1; }
  done
  echo "lane-new: --setup-only $setup_path"
  [ "$dry_run" -eq 1 ] || umask 077
  run ln -sfn "$env_local" "$setup_path/.env.local"
  run ln -sfn "$env_scheduler" "$setup_path/.env.scheduler"
  run bash "$script_dir/lane-cargo-config.sh" --at "$setup_path"
  run chmod -R go-rwx "$setup_path"
  echo "lane-new: setup applied — env symlinks, and cargo target $(bash "$script_dir/lane-cargo-config.sh" --print --at "$setup_path" 2>/dev/null || echo '?')"
  exit 0
fi

if [ -z "$name" ]; then
  usage >&2
  exit 2
fi
case "$name" in
  *[!a-z0-9-]*|"") echo "lane-new: name must be lowercase letters, digits and dashes: $name" >&2; exit 2 ;;
esac

# The lane lives beside the MAIN checkout, not beside whatever workspace you are
# standing in — every lane is a sibling of src/Culebraluxe-web. The common
# git dir is the main checkout's .git (a jj workspace is registered as a git
# worktree in this colocated repo, and they all share one object store), so it
# gives the parent directory no matter which lane invoked this.
common="$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)"
if [ -z "$common" ]; then
  echo "lane-new: not inside a git worktree" >&2
  exit 1
fi
main_root="$(dirname "$common")"
src_root="$(dirname "$main_root")"
lane_path="$src_root/lane-$name"
branch="lane/$name"

for f in "$env_local" "$env_scheduler"; do
  [ -f "$f" ] || { echo "lane-new: missing shared env file: $f" >&2; exit 1; }
done
if [ -e "$lane_path" ]; then
  echo "lane-new: $lane_path already exists" >&2
  exit 1
fi
# Two names can already be taken, and they are separate things: the git ref (a colocated jj repo keeps its
# bookmarks as git refs, so this catches a bookmark the roster may still name), and the workspace itself — a
# workspace whose bookmark was deleted would otherwise fail inside `jj workspace add` with a worse message.
if git -C "$main_root" show-ref --verify --quiet "refs/heads/$branch"; then
  echo "lane-new: bookmark $branch already exists" >&2
  exit 1
fi
if jj -R "$main_root" workspace list -T 'name ++ "\n"' 2>/dev/null | grep -qx "lane-$name"; then
  echo "lane-new: jj workspace lane-$name already exists" >&2
  exit 1
fi

echo "lane-new: $branch -> $lane_path"
if [ "$dry_run" -eq 0 ]; then
  umask 077
  # main@origin is the lane's base, so it is fetched rather than taken from whatever this checkout's `main`
  # happens to point at: a lane forks from the published trunk, never from a bookmark mid-flight.
  jj -R "$main_root" git fetch --branch main || { echo "lane-new: jj git fetch failed" >&2; exit 1; }
fi
# `jj workspace add` creates the directory (it refuses a non-empty one) and, in this colocated repo, registers a
# git worktree for it as well — so `git worktree list`, `git status` and the ten worktree-aware scripts keep
# seeing the lane instead of lying about what exists.
run jj -R "$main_root" workspace add "$lane_path" --name "lane-$name" -r main@origin || exit 1
# The bookmark the roster, the recovery tools and the landing recipe name. jj creates it UNTRACKED, which is the
# jj form of the old `git branch --unset-upstream`: a bare `jj git push` cannot aim at the trunk. Publishing it is
# `jj push --bookmark lane/<name>`, which tracks the remote bookmark on the first push.
run jj -R "$lane_path" bookmark create "$branch" -r main@origin || exit 1
run ln -sfn "$env_local" "$lane_path/.env.local"
run ln -sfn "$env_scheduler" "$lane_path/.env.scheduler"
# The lane's own build dir, so no two lanes queue on one cargo lock. The rule and the text live in
# scripts/lane-cargo-config.sh; this recipe only calls it, and it needs the lane to exist, which is
# why it comes after `workspace add`.
run bash "$script_dir/lane-cargo-config.sh" --at "$lane_path"
run chmod -R go-rwx "$lane_path"
if [ "$with_website" -eq 1 ]; then
  run bash -c "cd '$lane_path' && pnpm install --frozen-lockfile"
fi

if [ "$dry_run" -eq 1 ]; then
  echo "lane-new: dry run, nothing changed"
  exit 0
fi

echo
echo "lane-new: workspace $(basename "$lane_path") at change $(jj -R "$lane_path" --ignore-working-copy log -r @ --no-graph -T 'change_id.short()' 2>/dev/null || echo '?') on top of main@origin"
echo "lane-new: git sees the same tree, which is what the worktree-aware scripts need: $(git -C "$lane_path" rev-parse --show-toplevel 2>/dev/null || echo MISSING)"
echo "lane-new: next — add its row to docs/agent/LAYOUT.md, then work in $lane_path"
echo "lane-new: sync: jj git fetch && jj rebase -d main@origin    publish: jj push --bookmark $branch    land: docs/agent/JJ-INTERFACE.md"
