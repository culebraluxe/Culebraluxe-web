#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/jj-pre-push.sh — the git pre-push checks, for a `jj` push.
#
# WHY THIS EXISTS
#   `jj` runs no git hooks. Measured 2026-10-15: a `jj git push` whose range
#   touched `web/` returned in ~2 seconds with no compile and no hook output, so
#   the two things .githooks/pre-push owes a push — `Cargo.lock --locked`
#   (rule 5) and the wasm build of `web/ui` — silently did not happen. The hooks
#   are not broken; nothing runs them. This script is the stand-in until jj has
#   hooks of its own, or until the check lives somewhere a client cannot skip.
#
# WHAT IT DOES
#   The hook's two checks, in its order, then the push:
#     1. cargo metadata --locked --offline     — a lock out of date with a
#        Cargo.toml is refused (the deploy builds --locked).
#     2. web/ui compiles for wasm, when the checked range touches web/ui
#        (or `cargo check --workspace` when CULEBRALUXE_FULL_PUSH_CHECK=1).
#   Then `jj git push "$@"`: every argument is passed through, so `jj push`,
#   `jj push --branch x` and `jj push --dry-run` behave as they normally would.
#
# WHAT IT DOES NOT DO
#   It is not a guard. `jj git push` typed directly still bypasses it, exactly as
#   `git push --no-verify` always could. The only check a client cannot skip is
#   the one on a server (House Rules rule 9, `gates.yml`) — see
#   docs/agent/JJ-INTERFACE.md for whether that is currently running.
#
# THE RANGE IT CHECKS
#   `main@origin..main` by default; override with JJ_CHECK_FROM / JJ_CHECK_TO.
#   When the range cannot be resolved (a first push, a detached workspace) it
#   checks everything, which is what the git hook did for a brand-new ref.
#
# INSTALL (once per repo — the alias is written to .jj/repo/config.toml, which
# every workspace shares, and which is NOT committed because .jj/ is ignored):
#   bash scripts/jj-pre-push.sh --install
#
# RUN
#   jj push                                  # the alias, checks included
#   bash scripts/jj-pre-push.sh --dry-run    # same thing, pushes nothing
#   CULEBRALUXE_SKIP_BUILD_CHECK=1 jj push   # skips, and says so out loud
# ---------------------------------------------------------------------------
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ "${1:-}" = "--install" ]; then
  # Single-quoted on purpose: "$(jj root)" and "$@" must reach jj literally and
  # be expanded when the alias runs, not now. `--` before the user's arguments
  # makes it bash's $0, so "$@" inside is exactly what the agent typed.
  alias_value='["util","exec","--","bash","-c","bash \"$(jj root)/scripts/jj-pre-push.sh\" \"$@\"","--"]'
  if ! jj config set --repo aliases.push "$alias_value"; then
    echo "jj-pre-push: could not write the alias (is this a jj repo? is .jj writable?)" >&2
    exit 1
  fi
  echo "jj-pre-push: installed alias 'push' — 'jj push' now runs the checks first"
  echo "jj-pre-push: stored in .jj/repo/config.toml, so every workspace shares it; .jj/ is not committed"
  exit 0
fi

from="${JJ_CHECK_FROM:-main@origin}"
to="${JJ_CHECK_TO:-main}"
files=""
checked=""
if files="$(jj diff --name-only --from "$from" --to "$to" 2>/dev/null)"; then
  checked="$from..$to"
fi

ui_changed=0
rust_changed=0
if [ -z "$checked" ]; then
  ui_changed=1
  rust_changed=1
  echo "jj-pre-push: could not resolve ${from}..${to} — checking everything, as the git hook did for a new ref" >&2
else
  while IFS= read -r line; do
    case "$line" in web/ui/*) ui_changed=1 ;; esac
    for prefix in web/ middle/ db/ cli/ forge/ tests/ Cargo.toml Cargo.lock; do
      case "$line" in "$prefix"*) rust_changed=1 ;; esac
    done
  done <<< "$files"
fi

# --- Rule 5: Cargo.lock travels with Cargo.toml -----------------------------
if [ -f "$ROOT/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
  if ! cargo metadata --manifest-path "$ROOT/Cargo.toml" --locked --offline --format-version 1 >/dev/null 2>&1; then
    echo "House Rules (rule 5): Cargo.lock is out of date with a Cargo.toml. Run 'cargo metadata --manifest-path Cargo.toml' and commit Cargo.lock with it." >&2
    exit 1
  fi
fi

# --- the artifact's own build (the hook's second check) ---------------------
if [ "$ui_changed" = "1" ] || [ "$rust_changed" = "1" ]; then
  if [ -n "${CULEBRALUXE_SKIP_BUILD_CHECK:-}" ]; then
    echo "jj-pre-push: CULEBRALUXE_SKIP_BUILD_CHECK is set — the build check was SKIPPED. Name that in your report." >&2
  elif ! command -v cargo >/dev/null 2>&1; then
    echo "jj-pre-push: Rust changed but cargo is not on PATH, so the build could not be checked." >&2
    echo "  Install Rust, or push with CULEBRALUXE_SKIP_BUILD_CHECK=1 and say the check was skipped." >&2
    exit 1
  elif [ "$ui_changed" = "1" ]; then
    targets="$(rustup target list --installed 2>/dev/null || true)"
    case "$targets" in
      *wasm32-unknown-unknown*) ;;
      *) echo "jj-pre-push: web/ui changed but the wasm target is not installed. Run: rustup target add wasm32-unknown-unknown" >&2
         exit 1 ;;
    esac
    echo "jj-pre-push: checking web/ui compiles for wasm (what the deploy builds)..." >&2
    if ! cargo check --manifest-path "$ROOT/Cargo.toml" -p ui --features wasm --target wasm32-unknown-unknown --quiet; then
      echo "jj-pre-push: web/ui does not compile for wasm — refusing the push. The deploy builds exactly this." >&2
      echo "  Run it yourself with: pnpm ui:check" >&2
      exit 1
    fi
  elif [ -n "${CULEBRALUXE_FULL_PUSH_CHECK:-}" ]; then
    echo "jj-pre-push: checking the Rust workspace (CULEBRALUXE_FULL_PUSH_CHECK is set)..." >&2
    if ! cargo check --manifest-path "$ROOT/Cargo.toml" --workspace --all-targets --quiet; then
      echo "jj-pre-push: the Rust workspace does not compile — refusing the push. CI would refuse it too." >&2
      exit 1
    fi
  else
    echo "jj-pre-push: Rust changed — the workspace compile belongs to CI (gates.yml, on main)." >&2
    echo "  Locally: CULEBRALUXE_FULL_PUSH_CHECK=1 jj push   Slice tier: pnpm slice:check" >&2
  fi
fi

echo "jj-pre-push: checks done (range ${checked:-unresolved}: web/ui=$ui_changed rust=$rust_changed) — pushing" >&2
jj git push "$@"
