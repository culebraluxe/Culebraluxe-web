#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/agent-worker-once.sh — bounded Forge wake/run wrapper.
#
# ONE scheduled wake => recover an orphaned runtime if necessary, then
# repeatedly invoke `pnpm agent:work` until Forge is idle or a run fails.
# The database and `pnpm agent:work` own queue and orchestration semantics.
# This wrapper only heals stale process ownership and presses Forge again.
#
# IMPORTANT: launchd does not perform GitHub synchronization. Packet/story
# truth needed for execution must already be present in the checkout and/or
# durable Neon story fields before work is promoted Ready. Git synchronization
# remains an operator/control-plane concern, not a scheduler precondition.
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
STALE_AFTER_MINUTES="${AGENT_WORKER_STALE_AFTER_MINUTES:-60}"

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
  echo "[agent-worker] dry-run: runtime recovery and pnpm agent:work not invoked"
  inv_log "dry-run"
  exit 0
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "agent-worker: pnpm not found on PATH=$PATH" >&2
  inv_log "end: exit=127 pnpm-missing"
  exit 127
fi

if [ ! -f "$REPO_ROOT/.env.local" ]; then
  echo "agent-worker: .env.local missing; cannot recover/execute production control-plane work" >&2
  inv_log "end: exit=2 env-local-missing"
  exit 2
fi

inv_log "start: cwd=$REPO_ROOT max_passes=$MAX_PASSES stale_after_minutes=$STALE_AFTER_MINUTES"

# Industrial recovery boundary: if the previous worker/OpenCode/host died,
# recover its stale durable work BEFORE trying to claim anything new. The
# recovery command never deletes/reset/rebases the worker workspace. It closes
# the old run as Interrupted and requeues the SAME work item when retry budget
# remains. Any recovery failure stops this wake rather than risking a second
# writer or silently skipping durable state.
inv_log "recovery start stale_after_minutes=$STALE_AFTER_MINUTES"
APP_ENV=production node --env-file=.env.local node_modules/tsx/dist/cli.mjs \
  scripts/forge-runtime-recover.ts --stale-after "$STALE_AFTER_MINUTES"
recovery_rc=$?
inv_log "recovery end exit=$recovery_rc"
if [ "$recovery_rc" -ne 0 ]; then
  echo "agent-worker: stale runtime recovery failed; refusing to claim new work" >&2
  exit "$recovery_rc"
fi

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
