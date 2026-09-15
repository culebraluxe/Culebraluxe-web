#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/agent-worker-once.sh — bounded Forge wake/run wrapper.
#
# ONE scheduled wake => fast-forward the local control-plane checkout, then
# repeatedly invokes `pnpm agent:work` until Forge is idle or a run fails.
# Recovery, queue semantics, orchestration, Smith, Assay, publication and
# durable state all belong to Forge behind `pnpm agent:work`.
#
# This file is intentionally boring and stable because launchd executes a
# deployed copy outside ~/Documents (macOS TCC). The only Git responsibility
# here is a fail-closed `git pull --ff-only origin main` so unattended work
# never executes stale Forge code or stale packet contracts. Never stash,
# reset, rebase, force-update, or mutate worker branches here.
# ---------------------------------------------------------------------------

set -u

if [ -n "${AGENT_WORKER_REPO:-}" ]; then
  REPO_ROOT="$AGENT_WORKER_REPO"
else
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
fi
cd "$REPO_ROOT" || {
  echo "agent-worker: cannot change into repository root: $REPO_ROOT" >&2
  exit 1
}

ENV_OVERRIDE="$REPO_ROOT/.env.scheduler"
if [ -f "$ENV_OVERRIDE" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$ENV_OVERRIDE"
  set +a
fi

if [ -n "${AGENT_WORKER_PATH:-}" ]; then
  PATH="$AGENT_WORKER_PATH"
else
  PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
fi
export PATH

if [ -z "${HOME:-}" ]; then
  HOME="$(printf '%s' ~)"
  export HOME
fi

export AGENT_WORKER_ID="${AGENT_WORKER_ID:-scheduler}"
MAX_PASSES="${AGENT_WORKER_MAX_PASSES:-20}"

LOG_DIR="${AGENT_WORKER_LOG_DIR:-$HOME/Library/Logs/CulebraLuxe}"
INVOCATION_LOG="$LOG_DIR/agent-worker.invocations.log"
LOCK_DIR="$LOG_DIR/agent-worker.lock"
mkdir -p "$LOG_DIR" || exit 1

inv_log() {
  printf '%s %s\n' "$(date '+%Y-%m-%dT%H:%M:%S%z')" "$*" >> "$INVOCATION_LOG"
}

acquire_lock() {
  if mkdir "$LOCK_DIR" 2>/dev/null; then
    printf '%s\n' "$$" > "$LOCK_DIR/pid"
    return 0
  fi
  pid="$(cat "$LOCK_DIR/pid" 2>/dev/null || true)"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    return 1
  fi
  rm -rf "$LOCK_DIR" 2>/dev/null || true
  if mkdir "$LOCK_DIR" 2>/dev/null; then
    printf '%s\n' "$$" > "$LOCK_DIR/pid"
    return 0
  fi
  return 1
}

release_lock() {
  rm -rf "$LOCK_DIR" 2>/dev/null || true
}

if ! acquire_lock; then
  inv_log "skipped: another Forge worker invocation is still running"
  exit 0
fi
trap release_lock EXIT

if [ "${AGENT_WORKER_DRY_RUN:-0}" = "1" ]; then
  echo "[agent-worker] dry-run: pnpm agent:work not invoked"
  inv_log "dry-run"
  exit 0
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "agent-worker: pnpm not found on PATH=$PATH" >&2
  inv_log "end: exit=127 pnpm-missing"
  exit 127
fi

if ! command -v git >/dev/null 2>&1; then
  echo "agent-worker: git not found on PATH=$PATH" >&2
  inv_log "end: exit=127 git-missing"
  exit 127
fi

if [ ! -f "$REPO_ROOT/.env.local" ]; then
  # NAME THE RESOLVED ROOT. The deployed copy lives outside the repository, so if AGENT_WORKER_REPO is
  # unset its fallback ("<deployed dir>/..") points at a folder that is not a checkout — which is exactly
  # what a manual `pnpm agent:scheduler:run` hit on 2026-09-15 while the scheduled job was failing for a
  # different reason (TCC). Two different failures, one useless message.
  echo "agent-worker: .env.local missing; cannot execute production control-plane work" >&2
  echo "agent-worker: resolved repo root: $REPO_ROOT" >&2
  if [ -z "${AGENT_WORKER_REPO:-}" ]; then
    echo "agent-worker: AGENT_WORKER_REPO is not set and this copy is not inside the repository. Set it to" >&2
    echo "agent-worker: the checkout path, the way the LaunchAgent plist does." >&2
  fi
  inv_log "end: exit=2 env-local-missing repo=$REPO_ROOT"
  exit 2
fi

inv_log "start: cwd=$REPO_ROOT max_passes=$MAX_PASSES"

# Git is planned/runtime code truth; Neon is durable execution truth. Sync only
# the primary control-plane checkout and only by fast-forward. Any dirty or
# divergent checkout fails closed before Forge claims another work item.
#
# THE GIT ERROR IS NEVER DISCARDED. This check ran with `2>/dev/null` until 2026-09-15 and, when it failed,
# said only "found ''" — so a fail-closed check read as a mystery. It had in fact failed on EVERY scheduled
# tick since 2026-09-03 (463 invocations) because macOS TCC denies a launchd-spawned process access to
# ~/Documents: git could not read its own working directory ("fatal: Unable to read current working
# directory: Operation not permitted") and printed an empty branch, so the worker never reached
# `pnpm agent:work` and Forge was silently dead for twelve days. Print what git said, and say out loud that
# a permission failure is the likely cause.
branch_err_file="$(mktemp -t culebraluxe-branch.XXXXXX)"
branch="$(git branch --show-current 2>"$branch_err_file" || true)"
branch_err="$(tr '\n' ' ' < "$branch_err_file" 2>/dev/null | sed 's/[[:space:]]*$//')"
rm -f "$branch_err_file"
if [ "$branch" != "main" ]; then
  echo "agent-worker: expected control-plane checkout on main, found '$branch'" >&2
  if [ -n "$branch_err" ]; then
    echo "agent-worker: git said: $branch_err" >&2
    case "$branch_err" in
      *"Operation not permitted"*|*"Permission denied"*)
        echo "agent-worker: this is a macOS privacy (TCC) denial, not a git problem. The worker runs from" >&2
        echo "agent-worker: launchd, which cannot read ~/Documents unless the responsible binary is granted" >&2
        echo "agent-worker: Full Disk Access (System Settings > Privacy & Security > Full Disk Access)." >&2
        ;;
    esac
  fi
  inv_log "stop: checkout-not-main branch='$branch' git-error='$branch_err'"
  exit 2
fi
inv_log "git-sync: start origin/main"
if ! git pull --ff-only origin main; then
  echo "agent-worker: git fast-forward failed; Forge not started" >&2
  inv_log "stop: git-sync-failed"
  exit 2
fi
inv_log "git-sync: complete head=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

pass=1
while [ "$pass" -le "$MAX_PASSES" ]; do
  inv_log "pass=$pass start cmd=\"pnpm agent:work\""

  output_file="$(mktemp -t culebraluxe-forge-worker.XXXXXX)"
  pnpm agent:work 2>&1 | tee "$output_file"
  rc=${PIPESTATUS[0]}

  inv_log "pass=$pass end exit=$rc"

  if [ "$rc" -ne 0 ]; then
    rm -f "$output_file"
    inv_log "stop: Forge returned non-zero exit=$rc"
    exit "$rc"
  fi

  if grep -Eq '^no work($| —)' "$output_file"; then
    rm -f "$output_file"
    inv_log "idle: no work"
    exit 0
  fi

  rm -f "$output_file"
  pass=$((pass + 1))
done

inv_log "stop: max passes reached ($MAX_PASSES)"
echo "agent-worker: max passes reached ($MAX_PASSES); stopping until next scheduled wake" >&2
exit 0