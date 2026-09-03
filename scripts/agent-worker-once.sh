#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/agent-worker-once.sh — bounded Forge wake/run wrapper.
#
# ONE scheduled wake => repeatedly invokes `pnpm agent:work` until Forge is
# idle or a run fails. This mirrors the manual operator loop:
#   1. run Forge
#   2. if a clean lane queues the next lane, run Forge again immediately
#   3. if Forge reports no work, exit cleanly
#   4. if Forge exits non-zero, stop immediately and preserve the durable
#      Neon/Slack evidence for human inspection
#
# The database and `pnpm agent:work` own ALL queue/orchestration semantics:
# Ready discovery, hydration, single-worker enforcement, claiming, Smith,
# exact-candidate Assay, publication, Hold/Error state, and Slack notifications.
# This wrapper is only the unattended hand that presses the same button again.
#
# No secrets are embedded here. `pnpm agent:work` reads the gitignored
# `.env.local`. An optional untracked `.env.scheduler` may override local
# runtime settings.
#
# macOS TCC note: `pnpm agent:scheduler:install` deploys a copy of this wrapper
# outside ~/Documents and passes AGENT_WORKER_REPO so launchd can enter the repo.
#
# Env controls (all optional):
#   AGENT_WORKER_REPO       - repository root override
#   AGENT_WORKER_PATH       - replace default PATH
#   AGENT_WORKER_LOG_DIR    - log directory
#   AGENT_WORKER_ID         - worker identity (default scheduler)
#   AGENT_WORKER_DRY_RUN    - "1" logs/exits without running Forge
#   AGENT_WORKER_MAX_PASSES - runaway guard (default 20)
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

inv_log "start: cwd=$REPO_ROOT max_passes=$MAX_PASSES"

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

  # `agent:work` prints this only when there is nothing claimable/active.
  # Stop the wake cycle here. The launchd scheduler will check again in 3 min.
  if grep -Eq '^no work($| —)' "$output_file"; then
    rm -f "$output_file"
    inv_log "idle: no work"
    exit 0
  fi

  rm -f "$output_file"
  pass=$((pass + 1))

done

# Runaway protection only. Durable Forge state remains untouched; the next
# scheduled wake can continue normally.
inv_log "stop: max passes reached ($MAX_PASSES)"
echo "agent-worker: max passes reached ($MAX_PASSES); stopping until next scheduled wake" >&2
exit 0
