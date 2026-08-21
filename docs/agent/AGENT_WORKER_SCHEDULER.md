# Agent Worker Scheduler

Local scheduler for the production agent worker. Every **5 minutes** the
launchd LaunchAgent on this development Mac runs
`scripts/agent-worker-once.sh`, which invokes `pnpm agent:work` **exactly
once** and records operational output. The database owns all queue semantics;
the scheduler only wakes the worker.

## Architecture

```
PROD Story Board ── status -> Ready ──▶ DB trigger ──▶ agent_work_item (Ready)
                                                          │
    every 5 minutes: launchd ──▶ deployed wrapper copy
                                  (~/Library/Application Support/CulebraLuxe/
                                   agent-worker-once.sh)
                                      │ local no-overlap lock
                                      │ cd <repo>; pnpm agent:work (exactly once)
                                      ▼
                             claim AT MOST ONE story ──▶ DEV implementation
```

The LaunchAgent invokes a **deployed copy** of `scripts/agent-worker-once.sh`
because macOS TCC forbids launchd-spawned processes from executing files under
`~/Documents` (see "macOS TCC note" below). Manual
`pnpm agent:scheduler:run` uses the repo-resident wrapper directly.

Rules enforced here and by `pnpm agent:work` / the database:

- **One scheduled invocation = at most one story.** The wrapper never loops and
  never processes a second work item.
- Multiple `Ready` stories execute over **separate scheduler intervals**: a
  story is claimed, the run lifecycle moves it to `In Progress` / `Running`,
  the coding agent finishes it (`--finish`), and the next scheduled
  invocation may claim the next `Ready` item.
- The database (migration 025) remains the authoritative concurrency guard:
  the single-worker partial unique index plus the advisory lock mean two
  workers can never race into two active executions. The wrapper's local lock
  only prevents overlapping *local* invocations.

## Cadence

`StartInterval 300` in the LaunchAgent plist — one invocation every 5 minutes.
`RunAtLoad` runs once when the LaunchAgent loads (login or `install`).
LaunchAgents run only while a user is logged in — the appropriate model for a
development-host worker.

## Commands

| command | purpose |
|---|---|
| `pnpm agent:scheduler:install` | render the plist, install to `~/Library/LaunchAgents`, enable + bootstrap (idempotent; re-enables after `stop`) |
| `pnpm agent:scheduler:status` | loaded/enabled state, running worker (pid), last invocations |
| `pnpm agent:scheduler:run` | run the exact same wrapper once (manual single-story claim) |
| `pnpm agent:scheduler:stop` | **kill switch** — boot out now + persist disabled across login |
| `pnpm agent:scheduler:uninstall` | stop + delete the plist |

## Logs

All under `~/Library/Logs/CulebraLuxe/` (outside the repository):

| file | contents |
|---|---|
| `agent-worker.out.log` | stdout of scheduled runs (plist `StandardOutPath`) |
| `agent-worker.err.log` | stderr of scheduled runs (plist `StandardErrorPath`) |
| `agent-worker.invocations.log` | timestamped `start:` / `end: exit=<code>` per invocation |
| `agent-worker.lock/` | local no-overlap lock (holds the worker pid) |

Override the log location with `AGENT_WORKER_LOG_DIR`.

## Environment requirements

- Repository checked out at the path resolved from the wrapper (repo root).
- `node` and `pnpm` on PATH. launchd provides a minimal PATH, so the wrapper
  establishes one: `/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`.

## How the wrapper works

`scripts/agent-worker-once.sh`:

1. resolves the repository root (from its own path, or from the
   `AGENT_WORKER_REPO` environment supplied by the LaunchAgent)
2. `cd`s into the repository and sources the optional `.env.scheduler` override
3. establishes PATH / HOME
4. acquires a mkdir-based local lock (stale locks from dead pids are reclaimed)
5. logs `start:`, invokes `pnpm agent:work` exactly once, logs `end: exit=N`
6. exits with `pnpm agent:work`'s exit code (or `127` when pnpm is missing)
7. releases the lock via `trap` (even on failure)

If the lock is already held by a live invocation it logs `skipped:` and exits
`0` — the next scheduled interval retries.

## macOS TCC note (why the LaunchAgent uses a deployed copy)

macOS protects the `~/Documents`, `~/Desktop`, and `~/Downloads` folders with
TCC (privacy). launchd-spawned processes **cannot execute** files under those
folders — a job whose `ProgramArguments` points into `~/Documents` fails with
`Operation not permitted` (exit 126). `pnpm agent:scheduler:install` therefore
deploys a copy of the wrapper to
`~/Library/Application Support/CulebraLuxe/agent-worker-once.sh` (outside the
protected folders) and points the LaunchAgent at that copy. The plist supplies
`AGENT_WORKER_REPO` (the repository path), and the deployed copy `cd`s into the
repository itself, where reading files is permitted.

- Manual `pnpm agent:scheduler:run` keeps using the repo-resident wrapper
  (a terminal process already has access).
- After editing `scripts/agent-worker-once.sh`, re-run
  `pnpm agent:scheduler:install` so the deployed copy is refreshed.


## Failure behavior

Failures are boring and recoverable. The wrapper never retries aggressively
and never mutates Story Board state:

| failure | behavior |
|---|---|
| pnpm / node unavailable | wrapper logs the error, exits `127`, next interval retries |
| repository missing / unwritable | wrapper logs, exits non-zero, next interval retries |
| environment / `.env.local` missing | `pnpm agent:work` fails with its own error, exit code propagated |
| `agent:work` exits `Error` (infra) | exit code propagated; next interval retries |
| network unavailable | DB call fails; exit code propagated; next interval retries |
| a live invocation is still running | new invocation logs `skipped:` and exits `0` |
| a run is stale (no heartbeat) | `pnpm agent:work` reports it; `pnpm agent:work --recover` marks it terminal and unblocks the queue |

The scheduler never creates work items and never changes story state itself —
all lifecycle writes flow through the existing `pnpm agent:work` command.

## Run telemetry

While executing a story, the worker records observable progress with:

```
pnpm agent:work --progress <workItemId> --completion <0-100> --note "<milestone>" [--tests "<summary>"]
pnpm agent:work --finish   <workItemId> --result <outcome> --completion <n> --notes "<narrative>" --commit <hash> --tests "<summary>"
pnpm agent:work --error    <workItemId> --error-text "<why>"
pnpm agent:work --cancel   <workItemId> --note "<why>"
pnpm agent:work --recover  [--stale-after <minutes>]
```

Progress updates persist completion + a timestamped milestone note to
`storyboard_story_run` and refresh `agent_work_item.updated_at` (the heartbeat,
default stale threshold 60 minutes). Full semantics live in
`docs/agent/STORY_EXECUTION_CONTRACT.md` ("Run telemetry and lifecycle").

## Emergency stop / pause autonomous coding immediately

```
pnpm agent:scheduler:stop
```

This unloads the LaunchAgent now and persists a disabled flag, so no future
scheduled invocation occurs (including after the next login). It does **not**
touch Story Board data, queued work items, runs, or stories. A currently
running *scheduled* invocation is terminated by launchd; a manual
`pnpm agent:scheduler:run` in a terminal is unaffected.

Re-enable with `pnpm agent:scheduler:install`.

If Node itself is unavailable, use launchd directly:

```
launchctl bootout gui/$(id -u)/com.culebraluxe.agent-worker
launchctl disable gui/$(id -u)/com.culebraluxe.agent-worker
```

To inspect whether a worker is currently running:

```
pnpm agent:scheduler:status      # shows "running: yes (pid NNN)"
pgrep -fl agent-worker-once.sh   # direct process check
```

## Installing on a new machine

1. Ensure `node` + `pnpm` are installed and `.env.local` has the production
   control-plane credentials.
2. `pnpm install` (so `node_modules/tsx` and `pnpm agent:work` work).
3. Verify with `pnpm agent:scheduler:run` (prints `no work` when the queue is
   empty).
4. `pnpm agent:scheduler:install`.

The repository may live under the TCC-protected `~/Documents` — the installer
deploys the wrapper copy the LaunchAgent actually runs, so no manual TCC
grant is needed.

There is **no production migration** for this story — it changes only the
development worker host.

  Override entirely with `AGENT_WORKER_PATH`.
- Production DB access: `pnpm agent:work` loads the gitignored `.env.local`
  (`--env-file`), which supplies `DATABASE_URL_PROD` for the production
  control plane. **No secrets are embedded in tracked files or in the
  generated plist.**
- Optional untracked override file **`.env.scheduler`** (gitignored) may set
  `AGENT_WORKER_ID`, `AGENT_WORKER_PATH`, `AGENT_WORKER_LOG_DIR`, or other
  environment. Sourced (and exported) by the wrapper before PATH setup.
