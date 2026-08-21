#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/agent-worker-once.sh — single-invocation agent worker wrapper.
#
# ONE invocation => invokes `pnpm agent:work` exactly once => claims AT MOST
# ONE story. This script never loops and never processes a second work item;
# the next scheduled invocation may claim the next Ready item.
#
# The database (migration 025) owns all queue semantics — Ready discovery,
# single-worker enforcement, claiming, ordering, run lifecycle, story execution
# state. This wrapper ONLY:
#   1. changes into the repository root safely
#   2. establishes the runtime environment launchd does not provide
#   3. invokes exactly `pnpm agent:work`
#   4. propagates its exit code
#   5. records timestamped start/end information + exit code in a local log
#   6. guards against overlapping local invocations with a mkdir-based lock
#
# No secrets are embedded here. Production DB credentials are read by
# `pnpm agent:work` from the gitignored .env.local (--env-file). An optional
# untracked .env.scheduler file may override local runtime settings (see
# docs/agent/AGENT_WORKER_SCHEDULER.md).
#
# macOS TCC note: launchd-spawned processes cannot execute files under the
# TCC-protected ~/Documents folder, so `pnpm agent:scheduler:install` deploys
# a copy of this wrapper to ~/Library/Application Support/CulebraLuxe/ and the
# LaunchAgent invokes THAT copy. The deployed copy receives AGENT_WORKER_REPO
# (the repository path) through the plist environment, then cd's into the
# repository exactly as the repo-resident copy does.
#
# Env controls (all optional):
#   AGENT_WORKER_REPO   - repository root override (set by the deployed copy)
#   AGENT_WORKER_PATH   - replace the default PATH entirely (ops override)
#   AGENT_WORKER_LOG_DIR - log directory (default ~/Library/Logs/CulebraLuxe)
#   AGENT_WORKER_ID     - worker identity stamped on DB claims (default scheduler)
#   AGENT_WORKER_DRY_RUN - "1" logs/exits without invoking pnpm (plumbing test)
# ---------------------------------------------------------------------------

set -u

# --- resolve repository root from this script's own location (portable) ----
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

# --- optional local override file (untracked, documented) -------------------
ENV_OVERRIDE="$REPO_ROOT/.env.scheduler"
if [ -f "$ENV_OVERRIDE" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$ENV_OVERRIDE"
  set +a
fi

# --- establish runtime PATH (launchd provides a minimal PATH) ----------------
if [ -n "${AGENT_WORKER_PATH:-}" ]; then
  PATH="$AGENT_WORKER_PATH"
else
  PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
fi
export PATH

# --- HOME fallback ------------------------------------------------------------
if [ -z "${HOME:-}" ]; then
  HOME="$(printf '%s' ~)"
  export HOME
fi

export AGENT_WORKER_ID="${AGENT_WORKER_ID:-scheduler}"

# --- logs ---------------------------------------------------------------------
LOG_DIR="${AGENT_WORKER_LOG_DIR:-$HOME/Library/Logs/CulebraLuxe}"
INVOCATION_LOG="$LOG_DIR/agent-worker.invocations.log"
LOCK_DIR="$LOG_DIR/agent-worker.lock"

mkdir -p "$LOG_DIR" || {
  echo "agent-worker: cannot create log directory: $LOG_DIR" >&2
  exit 1
}

inv_log() {
  printf '%s %s\n' "$(date '+%Y-%m-%dT%H:%M:%S%z')" "$*" >> "$INVOCATION_LOG"
}

# --- local no-overlap lock (mkdir is atomic; stale-pid recovery) ---------------
acquire_lock() {
  if mkdir "$LOCK_DIR" 2>/dev/null; then
    printf '%s\n' "$$" > "$LOCK_DIR/pid"
    return 0
  fi
  # Lock held: reclaim only when the recorded pid is no longer alive.
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
  inv_log "skipped: another agent-worker invocation is still running"
  exit 0
fi
trap release_lock EXIT

inv_log "start: cwd=$REPO_ROOT cmd=\"pnpm agent:work\""

if [ "${AGENT_WORKER_DRY_RUN:-0}" = "1" ]; then
  echo "[agent-worker] dry-run: pnpm agent:work not invoked"
  rc=0
else
  if ! command -v pnpm >/dev/null 2>&1; then
    echo "agent-worker: pnpm not found on PATH=$PATH" >&2
    rc=127
  else
    pnpm agent:work
    rc=$?
  fi
fi

inv_log "end: exit=$rc"
exit "$rc"
