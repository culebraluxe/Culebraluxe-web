#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/opencode-session-stop.sh — stop a running agent TURN where it actually runs.
#
# WHY THIS EXISTS (measured 2026-10-11, OpenCode 2.0.26, one live Maestro tab)
# A Stop in the UI stops the CLIENT, not the turn. The turn belongs to the background server
# (`opencode serve --service`); the `opencode run` process it kills is only a reader of that
# turn's event stream. Measured inside one session (`ses_ee886b019ffevRX0zsLUs2bWAu`, the
# "RED Team batch 38 test assignment" tab, cwd lane-longcat):
#   * the stop landed as `InterruptError: All fibers interrupted without error` at 21:03:20 and
#     the operator pressed it again at 21:44:27,
#   * and `run=803ec8ab` was still spawning `cargo test … property_media__003__gallery` at 21:48,
#     and still streaming an assistant turn at 21:48:44 — 45 minutes after the first stop, with
#     its client long dead,
#   * while the tab read as stopped in the UI, two fresh `opencode run --session …` clients sat
#     blocked behind that turn (the operator's prompt was 20 s behind it and never served), and
#     the tab's own run records stayed `status: running`.
# So "I stopped it twice and it kept building tests" is not a model stubbornness problem; it is a
# stop that never reached the process that owns the work.
#
# The server has the route that does reach it: `POST /api/session/{sessionID}/interrupt`.
# Measured the same hour: `{"interrupted":true}`, the child `cargo test` processes gone within
# 15 s, an `idle` row written to `session_message`, `session_v2.idle_outcome` flipped
# `succeeded` → `interrupted`, and Maestro re-marked its two stuck runs `failed` by itself.
#
# WHY `opencode api` AND NOT `curl`. The password in service.json is not the auth this server
# accepts: measured 401 on `/openapi.json` and `/api/session` from 127.0.0.1 with
# `Authorization: Bearer <password>`. `opencode api` speaks whatever the running server expects,
# so this script goes through it instead of guessing at a header.
#
# WHAT IT WILL NOT DO. No `service restart` — that interrupts EVERY live session, and it stays the
# deliberate, manual `pnpm opencode:guard:repair`. No session delete, no config write, no
# `--force`, no git or worktree touch. Read-only except for the interrupt itself.
#
# MODES
#   --list          (DEFAULT) every session the SERVER reports as running, with the tab title,
#                   checkout and idle age from the local DB. This is the ground truth a UI Stop
#                   cannot show you.
#   --stop <ses_…>  interrupt that session server-side, then VERIFY it left the running set.
#                   Repeatable. A bare `ses_…` argument means `--stop ses_…`.
#
# FLAGS
#   --timeout <s>   ceiling for one `opencode api` call (default 30, OPENCODE_STOP_TIMEOUT)
#   --wait <s>      ceiling for the session to leave the running set (default 20, OPENCODE_STOP_WAIT)
#   --fail-over <n> with --list: exit n when at least one session is running (cron / gate use)
#   --quiet|-q      print problems only
#   --help|-h
#
# ENV: OPENCODE_BIN, OPENCODE_STATE_DIR, OPENCODE_DB_PATH — the names the service guard uses.
# EXIT: 0 every requested session is not running (including one that already was) · 1 a stop was
#       accepted but the session is still running after the ceiling · 2 refused (bad flags, no
#       CLI, unknown mode) · n the --fail-over value when --list found a running session
# ---------------------------------------------------------------------------
set -uo pipefail

OPENCODE_BIN="${OPENCODE_BIN:-}"
STATE_DIR="${OPENCODE_STATE_DIR:-$HOME/.local/state/opencode}"
SERVICE_JSON="$STATE_DIR/service.json"
DB_PATH="${OPENCODE_DB_PATH:-$HOME/.local/share/opencode/opencode.db}"
TIMEOUT="${OPENCODE_STOP_TIMEOUT:-30}"
WAIT="${OPENCODE_STOP_WAIT:-20}"

MODE=list
QUIET=0
FAIL_OVER=""
TARGETS=()
ACTIVE_IDS=()
ACTIVE_TYPES=()

say() { [ "$QUIET" = "1" ] || printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }
die() { warn "opencode-session-stop: $*"; exit 2; }

# Run a command with a hard ceiling. macOS ships no `timeout`, and `opencode api` talks to a
# server that can be wedged — the one thing a stop tool must not do is hang while stopping.
run_bounded() {
  local seconds="$1"; shift
  "$@" &
  local pid=$!
  ( sleep "$seconds"; kill -9 "$pid" 2>/dev/null ) >/dev/null 2>&1 &
  local watchdog=$!
  wait "$pid" 2>/dev/null
  local rc=$?
  kill "$watchdog" 2>/dev/null
  wait "$watchdog" 2>/dev/null
  return $rc
}

# The vendor CLI, by precedence: an explicit override, the vendor home layout, then PATH.
resolve_opencode_bin() {
  if [ -n "$OPENCODE_BIN" ] && [ -x "$OPENCODE_BIN" ]; then printf '%s' "$OPENCODE_BIN"; return 0; fi
  if [ -x "$HOME/.opencode/bin/opencode" ]; then printf '%s' "$HOME/.opencode/bin/opencode"; return 0; fi
  local on_path
  on_path="$(command -v opencode 2>/dev/null || true)"
  if [ -n "$on_path" ]; then printf '%s' "$on_path"; return 0; fi
  return 1
}

# What the running service says it is: `up <pid> <url>`, `stale <pid> <url>` when the published pid
# is gone, or `down` when nothing is published. A stale or down service owns no turn — the one case
# where "nothing was stopped" is the whole truth rather than a failure.
service_state() {
  python3 - "$SERVICE_JSON" <<'PY' 2>/dev/null
import json, os, sys
try:
    info = json.load(open(sys.argv[1]))
except Exception:
    print("down"); raise SystemExit(0)
pid = info.get("pid")
alive = False
if isinstance(pid, int):
    try:
        os.kill(pid, 0)
        alive = True
    except OSError:
        alive = False
print("%s %s %s" % ("up" if alive else "stale", pid, info.get("url", "?")))
PY
}

# Refresh the server's own running set into $ACTIVE, then load it into the two arrays. The parse
# reads a file rather than a pipe, so the arrays are set in THIS shell and not in a subshell.
read_active() {
  ACTIVE_IDS=()
  ACTIVE_TYPES=()
  run_bounded "$TIMEOUT" "$bin" api GET /api/session/active >"$ACTIVE" 2>/dev/null
  while IFS=$'\t' read -r id type; do
    [ -n "$id" ] || continue
    ACTIVE_IDS+=("$id")
    ACTIVE_TYPES+=("$type")
  done < <(python3 - "$ACTIVE" <<'PY' 2>/dev/null
import json, sys
try:
    d = json.load(open(sys.argv[1]))
except Exception:
    raise SystemExit(0)
data = d.get("data")
items = []
if isinstance(data, dict):
    items = list(data.items())
elif isinstance(data, list):
    items = [(v.get("id", "?"), v) for v in data if isinstance(v, dict)]
for key, val in items:
    kind = (val or {}).get("type", "?") if isinstance(val, dict) else "?"
    print("%s\t%s" % (key, kind))
PY
)
}

id_is_running() {
  local candidate
  for candidate in ${ACTIVE_IDS[@]+"${ACTIVE_IDS[@]}"}; do
    [ "$candidate" = "$1" ] && return 0
  done
  return 1
}

# Title, checkout and idle age for one session. Best-effort and read-only: an unreadable DB prints
# nothing rather than a guess, because a wrong title here is how a stop hits the wrong tab.
describe_session() {
  python3 - "$DB_PATH" "$1" <<'PY' 2>/dev/null || true
import os, sqlite3, sys, time
path, sid = sys.argv[1], sys.argv[2]
if not os.path.exists(path):
    raise SystemExit(0)
try:
    con = sqlite3.connect("file:%s?mode=ro" % path, uri=True)
except Exception:
    raise SystemExit(0)
row = None
for table in ("session_v2", "session"):
    try:
        row = con.execute(
            "select title, directory, time_updated from %s where id = ?" % table, (sid,)
        ).fetchone()
    except Exception:
        row = None
    if row:
        break
if not row:
    raise SystemExit(0)
title, directory, updated = row
age = ""
if isinstance(updated, int) and updated > 0:
    minutes = int((time.time() - updated / 1000) / 60)
    if minutes >= 0:
        age = "%dm idle" % minutes
print(" | ".join([part for part in (title or "(untitled)", directory or "?", age) if part]))
PY
}

# Fire the route and report what the server said about it. Sets INTERRUPT_VERDICT to
# true / false / unknown and INTERRUPT_REASON to the server's own words when it answered with
# something that is not an interrupt verdict (a 404 body, a wedged call). Measured upstream:
# `{"interrupted":false}` for a session that is already idle, both with exit 0; a missing session
# answers `SessionNotFoundError` + `HTTP 404 Not Found`. Those two must never read the same here.
INTERRUPT_VERDICT=""
INTERRUPT_REASON=""
interrupt_session() {
  local id="$1" resp rc parsed why
  resp="$(mktemp -t opencode-session-stop-resp)"
  INTERRUPT_VERDICT=""
  INTERRUPT_REASON=""
  run_bounded "$TIMEOUT" "$bin" api session.interrupt --param "sessionID=$id" >"$resp" 2>&1
  rc=$?
  parsed="$(python3 - "$resp" <<'PY' 2>/dev/null
import json, sys
try:
    raw = open(sys.argv[1]).read().strip()
except Exception:
    raise SystemExit(0)
try:
    d = json.loads(raw)
except Exception:
    raise SystemExit(0)
if isinstance(d, dict) and "interrupted" in d:
    print("true" if d["interrupted"] else "false")
else:
    print("unknown")
PY
)"
  case "$parsed" in
    true|false) INTERRUPT_VERDICT="$parsed" ;;
    *)
      INTERRUPT_VERDICT=unknown
      why="$(tr '\n' ' ' <"$resp" 2>/dev/null | cut -c1-160)"
      [ -n "$why" ] || why="no response from the server (exit $rc)"
      INTERRUPT_REASON="$why"
      ;;
  esac
  rm -f "$resp"
}

# Poll the server's own set until the id is gone, or the ceiling expires. Verified, not assumed.
wait_gone() {
  local id="$1" waited=0
  while [ "$waited" -lt "$WAIT" ]; do
    read_active
    if ! id_is_running "$id"; then return 0; fi
    sleep 2
    waited=$((waited + 2))
  done
  return 1
}

usage() {
  cat <<'USAGE'
usage: scripts/opencode-session-stop.sh [--list] [--quiet] [--fail-over <n>]
                                        [--stop <ses_…>]… [--timeout <s>] [--wait <s>]
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --list) MODE=list ;;
    --stop)
      [ $# -ge 2 ] || die "--stop needs a session id (ses_…)"
      MODE=stop
      TARGETS+=("$2")
      shift
      ;;
    --timeout)
      [ $# -ge 2 ] || die "--timeout needs a number of seconds"
      TIMEOUT="$2"
      shift
      ;;
    --wait)
      [ $# -ge 2 ] || die "--wait needs a number of seconds"
      WAIT="$2"
      shift
      ;;
    --fail-over)
      [ $# -ge 2 ] || die "--fail-over needs an exit code"
      FAIL_OVER="$2"
      shift
      ;;
    --quiet|-q) QUIET=1 ;;
    --help|-h) usage; exit 0 ;;
    ses_*)
      MODE=stop
      TARGETS+=("$1")
      ;;
    *) warn "opencode-session-stop: unknown argument: $1"; usage >&2; exit 2 ;;
  esac
  shift
done

main() {
  local bin state count i desc rc id interrupted
  bin="$(resolve_opencode_bin || true)"
  [ -n "$bin" ] || die "no opencode CLI found on PATH (set OPENCODE_BIN)"

  state="$(service_state)"
  case "$state" in
    up\ *) : ;;
    *)
      say "opencode-session-stop: service is $state — no turn can be running, nothing to stop"
      say "  service: pnpm opencode:guard:status"
      exit 0
      ;;
  esac

  ACTIVE="$(mktemp -t opencode-session-stop-active)"
  trap 'rm -f "${ACTIVE:-}"' EXIT

  case "$MODE" in
    list)
      read_active
      count=${#ACTIVE_IDS[@]}
      say "opencode-session-stop: service $state"
      say "opencode-session-stop: $count session(s) the server reports as running"
      i=0
      while [ "$i" -lt "$count" ]; do
        desc="$(describe_session "${ACTIVE_IDS[$i]}")"
        printf '  %s  %-8s %s\n' "${ACTIVE_IDS[$i]}" "${ACTIVE_TYPES[$i]}" "${desc:-no local record}"
        i=$((i + 1))
      done
      if [ "$count" -gt 0 ]; then
        say "  stop   : pnpm opencode:session:stop <ses_…>   (a UI Stop does not reach this turn)"
      fi
      if [ -n "$FAIL_OVER" ] && [ "$count" -gt 0 ]; then
        exit "$FAIL_OVER"
      fi
      exit 0
      ;;
    stop)
      read_active
      rc=0
      for id in ${TARGETS[@]+"${TARGETS[@]}"}; do
        desc="$(describe_session "$id")"
        if id_is_running "$id"; then
          say "stop   : $id  running — ${desc:-no local record}"
        else
          say "stop   : $id  not in /api/session/active — ${desc:-no local record}"
        fi
        interrupt_session "$id"
        interrupted="$INTERRUPT_VERDICT"
        say "  api  : interrupted=$interrupted${INTERRUPT_REASON:+  — $INTERRUPT_REASON}"
        if [ "$interrupted" = "unknown" ]; then
          # Fail closed: a call that did not answer, or one answered with a 404 body for a mistyped
          # id, is not a stop. `false` is different — the server is saying it had nothing running,
          # which is the state the caller asked for.
          rc=1
        fi
        if wait_gone "$id"; then
          say "  verify: gone from /api/session/active"
        else
          warn "  verify: STILL running after ${WAIT}s — the turn did not end"
          rc=1
        fi
      done
      exit "$rc"
      ;;
    *) die "unknown mode: $MODE" ;;
  esac
}

main "$@"
