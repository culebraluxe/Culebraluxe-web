#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/lane-cargo-config.sh — the one writer for a lane's cargo target dir.
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
# ---------------------------------------------------------------------------
set -u

usage() {
  cat <<'USAGE'
usage: scripts/lane-cargo-config.sh [--print] [--at <checkout>]

  Writes <checkout>/.cargo/config.toml naming that checkout's own cargo target dir,
  /Users/Shared/dev/build/rust-$(basename <checkout>). Idempotent: run it again in a lane
  that was copied or renamed, which is the only thing that file ever needs.

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

name="$(basename "${checkout%/}")"
case "$name" in
  lane-*) ;;
  *)
    echo "lane-cargo-config: $checkout is not a lane (a lane is a checkout named lane-<name>)." >&2
    echo "lane-cargo-config: the main checkout and Forge/Maestro sandboxes take the machine fallback," >&2
    echo "lane-cargo-config: target-dir = /Users/Shared/dev/build/rust-main, from ~/.cargo/config.toml." >&2
    exit 1
    ;;
esac

target_dir="/Users/Shared/dev/build/rust-$name"
config="$checkout/.cargo/config.toml"

if [ "$print_only" -eq 1 ]; then
  printf '%s\n' "$target_dir"
  exit 0
fi

mkdir -p "$checkout/.cargo"
cat > "$config" <<EOF
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
target-dir = "$target_dir"
EOF
# 600 on purpose: machine configuration inside a 700 tree, born closed rather than waiting for a
# `chmod -R go-rwx` pass to catch it.
chmod 600 "$config"
echo "lane-cargo-config: $name -> $target_dir"
