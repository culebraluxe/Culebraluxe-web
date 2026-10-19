#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/agent-worker-once.sh — bounded Forge wake/run wrapper.
#
# ONE scheduled wake => fast-forward the local control-plane checkout, then
# repeatedly invokes the Rust `forge-worker` binary until Forge is idle or a run fails.
# Recovery, queue semantics, orchestration, Smith, Assay, publication and
# durable state all belong to Forge behind the Rust `forge-worker` binary.
#
# This file is intentionally boring and stable because launchd executes a
# deployed copy outside ~/Documents (macOS TCC). Its only source-control
# responsibility is a fail-closed fast-forward of the control plane onto
# origin/main, so unattended work never executes stale Forge code or stale
# packet contracts: `jj git fetch` then `jj rebase -d main@origin` — the jj form
# of `git pull --ff-only origin main`, which stopped being answerable by git on
# 2026-10-15 when this checkout became a colocated jj workspace (measured that
# day: `git branch --show-current` answers "" here, because jj detaches HEAD at
# the working-copy commit). It never stashes, resets, force-updates or touches a
# lane's branch, and it still speaks git when a checkout has no `.jj` — the
# Forge worker sandboxes under forge/src/engine/worktree.rs still do not.
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

# REF NOISE GUARD. Git forbids spaces in ref names, so any file under .git/refs whose name contains a space
# is provably not a ref: it is a conflict copy left by the file-sync layer when the poller and the engine's
# worktrees wrote refs in the same moment (13 of them on 2026-09-16; one blocked a push as `bad object
# refs/remotes/origin/main 2` and would have blocked this wrapper's `git pull --ff-only` on an unattended
# tick, which is the exact failure this guard exists to prevent). Deleting them is always safe — they cannot
# be checked out, pushed or resolved. Objects are content-addressed and are deliberately untouched here.
REF_NOISE="$(find "$REPO_ROOT/.git/refs" -name '* *' -print 2>/dev/null | wc -l | tr -d ' ')"
if [ "${REF_NOISE:-0}" != "0" ]; then
  find "$REPO_ROOT/.git/refs" -name '* *' -delete 2>/dev/null
  echo "git-ref-noise: removed ${REF_NOISE} unresolved conflict copy(ies) from .git/refs"
fi
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

# rustup installs cargo here on macOS; launchd does not inherit the interactive shell PATH.
PATH="$HOME/.cargo/bin:$PATH"
export PATH

export AGENT_WORKER_ID="${AGENT_WORKER_ID:-scheduler}"
MAX_PASSES="${AGENT_WORKER_MAX_PASSES:-20}"

# Forge runs against PRODUCTION only, and the Rust worker hard-refuses anything else
# (forge/src/bin/forge_worker.rs:3 — `forge-worker must target production`, exit 2).
# The retired `pnpm agent:work` shim carried this; when the scheduler was switched to invoke
# the Rust worker directly the prefix was dropped, so every unattended tick since 2026-09-25
# died with exit=2 before claiming a single work item. Declared here rather than merely
# expected from the caller: an unattended tick must not be able to run against the wrong
# database. `.env.scheduler`, sourced above, may still override it deliberately.
APP_ENV="${APP_ENV:-production}"
EXECUTION_ENV="${EXECUTION_ENV:-PROD}"
export APP_ENV EXECUTION_ENV

# The shared machine log tree, outside every checkout so one directory answers "what did the jobs do"
# (docs/agent/LAYOUT.md, where the tree lives). `AGENT_WORKER_LOG_DIR` (set by the plist) still wins.
LOG_DIR="${AGENT_WORKER_LOG_DIR:-/Users/Shared/dev/build/logs}"
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

# The worker's own pool needs the production connection string, and this is the one entry point in the
# repository that could not find it. Every `forge`/`db-tool` command reaches `.env.local` through the CLI's
# `load_env()` (`cli/src/apple_sync.rs:68`); `forge-worker` is its own binary in the `forge` crate and calls
# nothing, so from 2026-09-25 an unattended tick died with `DatabaseUnavailable … DATABASE_URL_PROD is not
# configured` before claiming a single work item — and exited 1, which reads like a crashing run rather than an
# unstarted one. Read narrowly, not by sourcing the whole file: every variable this process holds is inherited by
# the agent subprocesses Forge spawns. `.env.scheduler`, sourced above, still wins if it sets the value.
if [ -z "${DATABASE_URL_PROD:-}" ] && [ -f "$REPO_ROOT/.env.local" ]; then
  DQ='"'
  SQ="'"
  line="$(sed -n -E 's/^[[:space:]]*(export[[:space:]]+)?DATABASE_URL_PROD=//p' "$REPO_ROOT/.env.local" | head -1)"
  line="${line%$'\r'}"
  line="${line%"$DQ"}"
  line="${line#"$DQ"}"
  line="${line%"$SQ"}"
  line="${line#"$SQ"}"
  DATABASE_URL_PROD="$line"
  unset line
  export DATABASE_URL_PROD
fi
if [ -z "${DATABASE_URL_PROD:-}" ]; then
  echo "agent-worker: DATABASE_URL_PROD is unset and .env.local carries none — refusing to start" >&2
  echo "agent-worker: Forge runs against PRODUCTION only; nothing is guessed here" >&2
  inv_log "stop: DATABASE_URL_PROD-missing"
  exit 2
fi

if [ "${AGENT_WORKER_DRY_RUN:-0}" = "1" ]; then
  echo "[agent-worker] dry-run: cargo run --manifest-path Cargo.toml -p forge --bin forge-worker -- not invoked"
  inv_log "dry-run"
  exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "agent-worker: cargo not found on PATH=$PATH" >&2
  inv_log "end: exit=127 cargo-missing"
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
# the Rust `forge-worker` binary and Forge was silently dead for twelve days. Print what git said, and say out loud that
# a permission failure is the likely cause.
branch_err_file="$(mktemp -t culebraluxe-branch.XXXXXX)"
branch="$(git branch --show-current 2>"$branch_err_file" || true)"
branch_err="$(tr '\n' ' ' < "$branch_err_file" 2>/dev/null | sed 's/[[:space:]]*$//')"
rm -f "$branch_err_file"

# THE CONTROL PLANE IS A jj WORKSPACE SINCE 2026-10-15, and git alone cannot answer the question above any more.
# Measured in this checkout on that day: `git branch --show-current` -> "" and `git symbolic-ref --short HEAD` ->
# "", because jj detaches HEAD at the working-copy commit. So the check below read `found ''` on every scheduled
# tick — 439 times between 2026-10-15T01:55 and 23:47 by `grep -c "checkout-not-main branch=''"` on
# build/logs/agent-worker.invocations.log — and the worker never reached the Rust binary. That is the same shape
# of bug the block above was written for (`found ''` reading as a mystery, Forge dead for twelve days), so the
# check now asks whichever interface owns the answer: jj when this checkout is a jj workspace, git otherwise.
# The intent is unchanged — the control plane must be ON main, not on a branch that wandered off — and in jj that
# is: `main` is a bookmark, and `main` is an ancestor of (or equal to) the working-copy commit `@`.
if [ -e "$REPO_ROOT/.jj" ] && ! command -v jj >/dev/null 2>&1; then
  echo "agent-worker: this checkout is a jj workspace but jj is not on PATH; Forge not started" >&2
  inv_log "stop: jj-missing"
  exit 2
fi
if [ -z "$branch" ] && [ -e "$REPO_ROOT/.jj" ]; then
  if jj --ignore-working-copy log -r 'main & ::@' --no-graph -T 'commit_id' 2>/dev/null | grep -q .; then
    branch="main"
  fi
fi
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
  # THE LOG LINE NAMES THE REAL FAULT (captain, 2026-09-16). The stderr above explained the TCC denial, but the
  # logged line still called it `checkout-not-main branch=''` — and the doctor reads this line, so for an hour
  # tonight a macOS privacy wall read as a wrong-branch problem. A stop reason that names the wrong fault costs
  # more than no reason at all.
  case "$branch_err" in
    *"Operation not permitted"*|*"Permission denied"*) stop_reason='unreadable-repo-dir (macOS TCC)';;
    *) stop_reason='checkout-not-main';;
  esac
  inv_log "stop: $stop_reason branch='$branch' git-error='$branch_err'"
  exit 2
fi
# THE FAST-FORWARD, IN THE INTERFACE THAT OWNS IT. `git pull --ff-only origin main` cannot be the answer in a
# colocated jj workspace: HEAD is detached (there is no branch to fast-forward) and the working copy belongs to
# jj, not to git's index — running git's pull here writes refs jj owns, which is how two checkouts end up
# disagreeing about where main is. The jj form keeps every intent of the old line and fails closed the same way:
# `jj bookmark set main -r main@origin` refuses a backwards or sideways move (that is the `--ff-only` refusal),
# `jj rebase -d main@origin` is what moves this checkout's working copy onto the new trunk, and a conflict after
# it is the jj form of "the pull would have overwritten something you have".
if [ -e "$REPO_ROOT/.jj" ]; then
  inv_log "jj-sync: start origin/main"
  if ! jj git fetch; then
    echo "agent-worker: jj git fetch failed; Forge not started" >&2
    inv_log "stop: jj-sync-fetch-failed"
    exit 2
  fi
  if ! jj bookmark set main -r main@origin; then
    echo "agent-worker: control-plane main is not a fast-forward of origin/main; Forge not started" >&2
    inv_log "stop: jj-sync-not-fast-forward"
    exit 2
  fi
  if ! jj rebase -d main@origin; then
    echo "agent-worker: rebasing the control plane onto origin/main failed; Forge not started" >&2
    inv_log "stop: jj-sync-rebase-failed"
    exit 2
  fi
  if [ "$(jj --ignore-working-copy log -r @ --no-graph -T 'if(conflict, "conflict", "ok")' 2>/dev/null)" != "ok" ]; then
    echo "agent-worker: the control-plane working copy is in conflict after the sync; Forge not started" >&2
    inv_log "stop: jj-sync-conflict"
    exit 2
  fi
  inv_log "jj-sync: complete change=$(jj --ignore-working-copy log -r @ --no-graph -T 'commit_id.short()' 2>/dev/null || echo unknown)"
else
  inv_log "git-sync: start origin/main"
  if ! git pull --ff-only origin main; then
    echo "agent-worker: git fast-forward failed; Forge not started" >&2
    inv_log "stop: git-sync-failed"
    exit 2
  fi
  inv_log "git-sync: complete head=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
fi

pass=1
while [ "$pass" -le "$MAX_PASSES" ]; do
  inv_log "pass=$pass start cmd=\"cargo run --manifest-path Cargo.toml -p forge --bin forge-worker --\""

  output_file="$(mktemp -t culebraluxe-forge-worker.XXXXXX)"
  if [ -n "${AGENT_WORKER_BIN:-}" ]; then
    # Deployed release artifact (scripts/deploy-forge-prod.sh): no cargo compile on the wake path.
    if [ ! -x "$AGENT_WORKER_BIN" ]; then
      echo "agent-worker: AGENT_WORKER_BIN is not executable: $AGENT_WORKER_BIN" >&2
      rm -f "$output_file"
      inv_log "stop: worker-bin-missing $AGENT_WORKER_BIN"
      exit 2
    fi
    "$AGENT_WORKER_BIN" 2>&1 | tee "$output_file"
  else
    cargo run --manifest-path Cargo.toml -p forge --bin forge-worker -- 2>&1 | tee "$output_file"
  fi
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