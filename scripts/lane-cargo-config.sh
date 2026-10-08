#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/lane-cargo-config.sh — the one writer for a checkout's cargo target dir.
#
# WHY THIS EXISTS
#   Since 2026-10-07 every checkout builds into a target directory of its OWN:
#   /Users/Shared/dev/build/rust-<checkout>. One shared directory put every lane behind
#   one cargo lock — cargo holds an exclusive lock on <target>/.cargo-lock for the
#   length of a build — so the second build of the day sat waiting with no children
#   while the CPU idled between builds, and sharing it deduplicated nothing: the
#   shared 9.9 GB held 45 copies of `sqlx_postgres`, 865 rlibs for 261 crates, because
#   cargo keys an artifact by feature/target/profile and not by path. The rule, the
#   measurements and what it replaced: docs/agent/LAYOUT.md, "One target directory per
#   checkout".
#
#   WHICH DIRECTORY, BY KIND OF CHECKOUT (2026-10-08: "fix the build so everyone has their own build")
#     lane-<name>          -> /Users/Shared/dev/build/rust-lane-<name>, one per lane, outside the tree.
#     the main checkout    -> refused here: it takes the machine fallback, build/rust-main, from
#                             ~/.cargo/config.toml — it outlives everything and is named once, not per run.
#     anything else        -> <checkout>/.cargo-target, INSIDE that checkout's own tree: a Forge/Maestro
#                             sandbox (`git worktree add`, forge/src/engine/worktree.rs), the detached
#                             integration proof (`with_detached_checkout`, same file), anything a command makes.
#   Why a sandbox builds inside its own tree when no lane may: a sandbox is DISPOSABLE — `git worktree
#   remove` ends it — so a target there is deleted with the checkout it belongs to. Anywhere else it
#   outlived its owner: the shared dir, and then each build/rust-wt-* dir this script briefly wrote, kept
#   gigabytes of artifacts for worktrees that no longer existed. This is not the shared directory coming
#   back — each sandbox still has a target of its own, so two sandboxes never queue on one cargo lock.
#
#   WHY `.cargo-target` AND NOT `target` — the landmine this name steps over. `scripts/rust-ui-build.sh:42`
#   reads cargo's own answer `"<checkout>/target"` as "we are in the container" and sends the wasm build to
#   `/target`, which is read-only on macOS (it died with `Read-only file system (os error 30) at path
#   "/targetXXXXXX"`, 2026-10-04). So an in-tree build dir that is *named* `target` looks configured and
#   works for `cargo check` while breaking `pnpm ui:build` and the deploy — the failure MEMORY.md records
#   as the reason the per-checkout directories are named by a file at all. The container heuristic is
#   triggered by the name, not by who wrote it, so a disposable checkout gets a name that is not that one.
#   Do not "simplify" this back to `target`.
#
# WHY A FILE, AND WHY THIS FILE
#   A file rather than an export, because cargo reads config files itself, shell or not:
#   the agents running on this Mac carry no CARGO_TARGET_DIR at all, and cargo's own
#   default (`<checkout>/target`) is not usable here — `scripts/rust-ui-build.sh` maps
#   that answer to `/target`, which is read-only on macOS. A checkout-local config is
#   also the only mechanism that can be per-checkout: a relative `target-dir` resolves
#   against the config file's own directory, so a single global rule cannot do it.
#
#   The text lives here and nowhere else, so the recipe (`scripts/lane-new.sh`, which
#   calls this for every new lane) and a lane that was copied or renamed cannot drift.
#   The file it writes is untracked and git-ignored — it names an absolute path.
#
# RUN
#   bash scripts/lane-cargo-config.sh                # this checkout (git's top level)
#   bash scripts/lane-cargo-config.sh --print        # print the target dir, change nothing
#   bash scripts/lane-cargo-config.sh --at <dir>     # another checkout; lane-new.sh uses this
#
#   Nothing calls this by hand for a sandbox: .githooks/post-checkout runs it once for every checkout git
#   creates, so `git worktree add` names that sandbox's target dir without anyone remembering to.
# ---------------------------------------------------------------------------
set -u

usage() {
  cat <<'USAGE'
usage: scripts/lane-cargo-config.sh [--print] [--at <checkout>]

  Writes <checkout>/.cargo/config.toml naming that checkout's own cargo target dir:
  /Users/Shared/dev/build/rust-lane-<name> for a lane, <checkout>/.cargo-target for a
  disposable sandbox. Idempotent: run it again in a lane that was copied or renamed,
  which is the only thing that file ever needs. The main checkout is refused — it takes
  build/rust-main from ~/.cargo/config.toml, and this script is not going to be its
  second answer.

  --print          print the target dir it would write, and write nothing
  --at <checkout>  act on <checkout> instead of the checkout you are standing in
USAGE
}

checkout=""
print_only=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --print)   print_only=1 ;;
    --at)      shift; checkout="${1:-}" ;;
    -h|--help) usage; exit 0 ;;
    *)         echo "lane-cargo-config: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

if [ -z "$checkout" ]; then
  checkout="$(git rev-parse --show-toplevel 2>/dev/null || true)"
fi
if [ -z "$checkout" ]; then
  echo "lane-cargo-config: not inside a git checkout (pass --at <checkout>)" >&2
  exit 1
fi
case "$checkout" in
  /*) ;;
  *) echo "lane-cargo-config: <checkout> must be an absolute path: $checkout" >&2; exit 2 ;;
esac
if [ "$print_only" -eq 0 ] && [ ! -d "$checkout" ]; then
  echo "lane-cargo-config: no such checkout: $checkout" >&2
  exit 1
fi

# Which kind of checkout is this? The answer decides the target dir, and this is the one place that decides.
name="$(basename "${checkout%/}")"
kind=""
case "$name" in
  lane-*) kind="lane" ;;
esac
if [ -z "$kind" ]; then
  # `.git` as a DIRECTORY is the main checkout; in a linked worktree it is a file pointing at the common dir.
  if [ -d "$checkout/.git" ]; then kind="main"; else kind="sandbox"; fi
fi

case "$kind" in
  main)
    echo "lane-cargo-config: $checkout is the main checkout, which takes the machine fallback:" >&2
    echo "lane-cargo-config:   target-dir = /Users/Shared/dev/build/rust-main, from ~/.cargo/config.toml." >&2
    exit 1
    ;;
  lane) target_dir="/Users/Shared/dev/build/rust-$name" ;;
  *)    target_dir="$checkout/.cargo-target" ;;
esac
config="$checkout/.cargo/config.toml"

if [ "$print_only" -eq 1 ]; then
  printf '%s\n' "$target_dir"
  exit 0
fi

mkdir -p "$checkout/.cargo"
# The delimiter is QUOTED on purpose, and that is load-bearing: this body is prose, and an UNQUOTED heredoc
# *executes* what it finds — the first draft of the text below quoted commands in backticks, so writing a
# sandbox's config ran `git worktree add`, `pnpm ui:build` and `cargo check` on the way out (2026-10-08:
# `git worktree add` went from 0s to 32s, git's usage text landed in the post-checkout hook's output, and a
# killed run left a 0-byte config). Quoted, nothing in the body is expanded, whatever the prose does later.
# The one value that must expand is appended below by printf.
if [ "$kind" = "sandbox" ]; then
  {
    cat <<'EOF'
# Sandbox-local cargo build output, INSIDE this disposable checkout on purpose. Written by
# scripts/lane-cargo-config.sh — run that script again if this checkout is moved or copied. Untracked and
# git-ignored (/.cargo-target/ in the repository and in scripts/wip-snapshot.sh), so it is never a dirty
# file and never a 5-GB snapshot.
#
# Why inside the tree when no lane may build inside its own: this checkout does not outlive its story.
# `git worktree add` makes it and `git worktree remove` takes it away, so its build output goes with it —
# while a shared directory (or a build/rust-wt-* one) kept gigabytes of artifacts for worktrees that were
# long gone. Each sandbox still has a target of its OWN, so two sandboxes never queue on one cargo lock.
#
# Why the dir is not called `target`: scripts/rust-ui-build.sh reads cargo's answer "<checkout>/target" as
# "we are in the container" and builds into /target, read-only on macOS. Renaming this would break
# `pnpm ui:build` while `cargo check` stayed green. docs/agent/LAYOUT.md, "One target directory per checkout".
[build]
EOF
    printf 'target-dir = "%s"\n' "$target_dir"
  } > "$config"
else
  {
    cat <<'EOF'
# Lane-local cargo build output. Written by scripts/lane-cargo-config.sh — run that script again if this
# lane is copied or renamed, because this file is the one thing in a lane that names it. Untracked and
# git-ignored: it names an absolute path on this Mac.
#
# Why each lane has its own instead of sharing one: cargo holds an exclusive lock on the target directory
# for the length of a build, so one shared dir serialises every lane on the machine, and sharing
# deduplicated nothing (measured 2026-10-07: 9.9 GB holding 45 copies of sqlx_postgres).
# Do not point this at another lane or at build/rust-main. docs/agent/LAYOUT.md,
# "One target directory per checkout".
[build]
EOF
    printf 'target-dir = "%s"\n' "$target_dir"
  } > "$config"
fi
# 600 on purpose: machine configuration inside a 700 tree, born closed rather than waiting for a
# `chmod -R go-rwx` pass to catch it.
chmod 600 "$config"
echo "lane-cargo-config: $name -> $target_dir"
