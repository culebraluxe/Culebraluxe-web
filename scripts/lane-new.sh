#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/lane-new.sh — create a lane: one code-only worktree, on its own branch.
#
# WHY THIS EXISTS
#   Eleven lanes were made by hand on 2026-10-03 and the recipe has four steps
#   that are easy to get subtly wrong, each with a cost that only shows up later:
#   a copy of `.env.local` instead of a symlink (four rotation points for one set
#   of secrets, in lanes nobody looks at), an upstream left pointing at
#   `origin/main` (a bare `git push` then aims at the trunk), modes left at 755
#   inside a 700 tree, and `pnpm install` run "to be safe" (a 931 MB lane that
#   needs no website). This script is that recipe with the four traps closed.
#
# WHAT A LANE IS
#   Code only. ~84 MB, no node_modules, no target directory: cargo builds into the
#   one shared CARGO_TARGET_DIR (/Users/Shared/dev/build/rust, from
#   ~/.cargo/config.toml) and `pnpm install` is a per-lane decision, not part of
#   setup — a lane without node_modules still compiles, tests, commits and lands.
#
# WHAT IT DOES
#   1. git worktree add <sibling>/lane-<name> -b lane/<name> origin/main
#   2. ln -sfn the two shared env files into the lane root
#   3. git branch --unset-upstream
#   4. chmod -R go-rwx the lane (umask 077 is set first, so new files start closed)
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
# ---------------------------------------------------------------------------
set -u

usage() {
  cat <<'USAGE'
usage: scripts/lane-new.sh <name> [--with-website] [--dry-run]

  <name>            lowercase letters, digits and dashes: lane-<name> is created
                    beside the main checkout, on branch lane/<name>
  --with-website    also run `pnpm install --frozen-lockfile` in the new lane
                    (~30 MB real, ~847 MB as du counts it) for tailwind / pnpm build
  --dry-run         print the commands and exit without changing anything
USAGE
}

name=""
with_website=0
dry_run=0
for arg in "$@"; do
  case "$arg" in
    --with-website) with_website=1 ;;
    --dry-run)      dry_run=1 ;;
    -h|--help)      usage; exit 0 ;;
    -*)             echo "lane-new: unknown option: $arg" >&2; usage >&2; exit 2 ;;
    *)              name="$arg" ;;
  esac
done

if [ -z "$name" ]; then
  usage >&2
  exit 2
fi
case "$name" in
  *[!a-z0-9-]*|"") echo "lane-new: name must be lowercase letters, digits and dashes: $name" >&2; exit 2 ;;
esac

# The lane lives beside the MAIN checkout, not beside whatever worktree you are
# standing in — all eleven lanes are siblings of src/Culebraluxe-web. The common
# git dir is the main checkout's .git (worktrees share one object store), so it
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

env_local="/Users/Shared/dev/.env.local"
env_scheduler="/Users/Shared/dev/.env.scheduler"

for f in "$env_local" "$env_scheduler"; do
  [ -f "$f" ] || { echo "lane-new: missing shared env file: $f" >&2; exit 1; }
done
if [ -e "$lane_path" ]; then
  echo "lane-new: $lane_path already exists" >&2
  exit 1
fi
if git -C "$main_root" show-ref --verify --quiet "refs/heads/$branch"; then
  echo "lane-new: branch $branch already exists" >&2
  exit 1
fi

run() {
  echo "  + $*"
  [ "$dry_run" -eq 1 ] || "$@"
}

echo "lane-new: $branch -> $lane_path"
if [ "$dry_run" -eq 0 ]; then
  umask 077
  git -C "$main_root" fetch -q origin main || { echo "lane-new: fetch failed" >&2; exit 1; }
fi
run git -C "$main_root" worktree add "$lane_path" -b "$branch" origin/main || exit 1
run ln -sfn "$env_local" "$lane_path/.env.local"
run ln -sfn "$env_scheduler" "$lane_path/.env.scheduler"
run git -C "$lane_path" branch --unset-upstream
run chmod -R go-rwx "$lane_path"
if [ "$with_website" -eq 1 ]; then
  run bash -c "cd '$lane_path' && pnpm install --frozen-lockfile"
fi

if [ "$dry_run" -eq 1 ]; then
  echo "lane-new: dry run, nothing changed"
  exit 0
fi

echo
echo "lane-new: $branch at $(git -C "$lane_path" rev-parse --short HEAD), $(git -C "$lane_path" status --short | wc -l | tr -d ' ') dirty files"
echo "lane-new: next — add its row to docs/agent/LAYOUT.md, then work in $lane_path"
echo "lane-new: land with: git push origin $branch:main   (after: git fetch origin main && git rebase origin/main)"
