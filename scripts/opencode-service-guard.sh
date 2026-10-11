#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/opencode-service-guard.sh — stop `opencode models` from reading as EMPTY,
# and stop every model picker that trusts it (Maestro, workers) from inheriting that.
#
# WHY THIS EXISTS (measured 2026-10-11, OpenCode 2.0.26)
# `opencode models` does not build its list in-process. The CLI hands the question to a
# background server — `opencode serve --service` — whose {pid,url,password} is published in
#   ~/.local/state/opencode/service.json
# and that server owns BOTH halves of the answer:
#   * the models.dev catalogue, persisted as a SQLite row
#       opencode.db -> kv['models-dev:catalog'] = {updatedAt, digest, body}
#     where digest == sha256(body) and body is the models.dev api.json (226 providers,
#     120 opencode models, 36 of them `-free`, verified 2026-10-10 21:52), and
#   * the live served roster, which filters that catalogue down to the free lane actually
#     being served at that moment (36 catalogue ids -> the 11 `opencode/*` ids printed).
#
# Three measured consequences, each verified by experiment rather than inferred:
#   1. OPENCODE_MODELS_PATH, OPENCODE_MODELS_URL, OPENCODE_DISABLE_MODELS_FETCH and
#      OPENCODE_DB change NOTHING for `opencode models` while that server is up — the client
#      never reads them (corrupting each left the output byte-identical). The service is the
#      only writer that counts, so "point the CLI at a pinned file" is NOT a fix.
#   2. The failure FAILS OPEN: when the service resolves no catalogue/roster, the command
#      prints an empty list and exits 0. There is no error text anywhere.
#   3. A fresh service is healthy and the same service goes bad later. Observed today: the
#      roster answered normally from the service started at 20:47:59 (right after the update)
#      and read empty two hours later with the service still up — exactly the
#      "it worked when I updated, and 2 hours later the list is empty" report.
#
# So this guard treats the roster as a SERVICE health fact, not a config file: it snapshots a
# known-good roster and catalogue row while the service is healthy, checks those invariants
# afterwards, and can put the catalogue row back and restart the service when they break.
# The roster snapshot is also the trusted baseline the Maestro guard falls back to when the
# live call is empty, so a broken service can no longer freeze model-pin repair.
#
# MODES
#   --check      (DEFAULT) if the service is not running, start it — a stopped service cannot be
#                interrupted, unlike a restart — then wait for the roster to warm up, validate
#                the catalogue row and the roster, and snapshot when healthy. Exit 1 when
#                unhealthy. Safe in a gate; this is what the login agent runs every 5 minutes.
#   --snapshot   record the current roster and catalogue row as the last known good
#   --repair     put the catalogue row back if it is unusable, `opencode service restart`,
#                then re-check. A restart interrupts live sessions, so this is deliberate and
#                manual; the login agent never calls it.
#   --status     human-readable report (service, catalogue, roster, snapshot age)
#   --install    install a LaunchAgent that runs --check every 5 minutes and notifies
#   --uninstall  remove that LaunchAgent
#
# FLAGS
#   --quiet      print problems only
#   --notify     post a macOS notification when the check fails (used by the login agent)
#   --force      with --repair: restart even when the roster already looks healthy
#
# ENV OVERRIDES: OPENCODE_BIN, OPENCODE_STATE_DIR, OPENCODE_DB_PATH, OPENCODE_GUARD_DIR,
#                OPENCODE_GUARD_MIN_FREE (default 3), OPENCODE_GUARD_MIN_PROVIDERS (default 100),
#                OPENCODE_GUARD_AUTOSTART (default 1: --check starts a STOPPED service),
#                OPENCODE_GUARD_START_TIMEOUT (default 25s), OPENCODE_GUARD_WARMUP (default 30s)
#
# EXIT: 0 healthy · 1 unhealthy (or snapshot missing) · 2 refused (bad flags, no CLI)
# ---------------------------------------------------------------------------

set -uo pipefail

OPENCODE_BIN="${OPENCODE_BIN:-}"
STATE_DIR="${OPENCODE_STATE_DIR:-$HOME/.local/state/opencode}"
SERVICE_JSON="$STATE_DIR/service.json"
DB_PATH="${OPENCODE_DB_PATH:-$HOME/.local/share/opencode/opencode.db}"
GUARD_DIR="${OPENCODE_GUARD_DIR:-$STATE_DIR/guard}"
ROSTER_SNAPSHOT="$GUARD_DIR/roster.txt"
CATALOG_SNAPSHOT="$GUARD_DIR/catalog.json"
META_SNAPSHOT="$GUARD_DIR/meta.json"
CATALOG_KEY="models-dev:catalog"
MIN_FREE="${OPENCODE_GUARD_MIN_FREE:-3}"
MIN_PROVIDERS="${OPENCODE_GUARD_MIN_PROVIDERS:-100}"
LOG_DIR="${AGENT_WORKER_LOG_DIR:-/Users/Shared/dev/build/logs}"
PLIST_LABEL="com.culebraluxe.opencode-service-guard"
PLIST_DEST="$HOME/Library/LaunchAgents/$PLIST_LABEL.plist"

MODE=check
QUIET=0
NOTIFY=0
FORCE=0

usage() {
  cat <<'USAGE'
usage: scripts/opencode-service-guard.sh [--check|--snapshot|--repair|--status|--install|--uninstall]
                                         [--quiet] [--notify] [--force]
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --check) MODE=check ;;
    --snapshot) MODE=snapshot ;;
    --repair) MODE=repair ;;
    --status) MODE=status ;;
    --install) MODE=install ;;
    --uninstall) MODE=uninstall ;;
    --quiet|-q) QUIET=1 ;;
    --notify) NOTIFY=1 ;;
    --force) FORCE=1 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "opencode-service-guard: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

say() { [ "$QUIET" = "1" ] || printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }
die() { warn "opencode-service-guard: $*"; exit 2; }

# Run a command with a hard ceiling. macOS ships no `timeout`, and a hung `service restart`
# is the one thing that must never wedge a login agent.
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

# The live roster, captured to a file. Returns the count of `opencode/*` ids on stdout:
# that count — not the total — is the health signal, because the operator's own providers
# (deepseek/*, meta/*) keep printing while the free lane is gone.
fetch_roster() {
  local bin="$1" out="$2"
  "$bin" models 2>/dev/null | grep -E '^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$' | sort -u > "$out"
  grep -c '^opencode/' "$out" 2>/dev/null || true
}

# What the running service says it is. Prints `pid version url started` or `down`.
service_state() {
  python3 - "$SERVICE_JSON" <<'PY' 2>/dev/null
import json, os, sys, subprocess
path = sys.argv[1]
try:
    info = json.load(open(path))
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
started = ""
if alive:
    out = subprocess.run(["/bin/ps", "-o", "lstart=", "-p", str(pid)],
                         capture_output=True, text=True).stdout.strip()
    started = out
print("%s %s %s %s" % ("up" if alive else "stale",
                       info.get("pid", "?"), info.get("version", "?"),
                       info.get("url", "?") + ("" if not started else " since " + started)))
PY
}

# Inspect the persisted models.dev catalogue row. Prints one status line, then exits
# 0 when the row is structurally sound, 1 when it is missing or unusable. The digest is
# checked because it is what ties `body` to a real models.dev response: a truncated or
# half-written body must never be mistaken for a catalogue.
catalog_state() {
  python3 - "$DB_PATH" "$CATALOG_KEY" "$MIN_PROVIDERS" <<'PY' 2>/dev/null
import hashlib, json, sqlite3, sys, datetime
db, key, min_providers = sys.argv[1], sys.argv[2], int(sys.argv[3])
if not __import__("os").path.exists(db):
    print("MISSING db=%s" % db); raise SystemExit(1)
try:
    con = sqlite3.connect("file:%s?mode=ro" % db, uri=True, timeout=3)
    row = con.execute("select value, time_updated from kv where key=?", (key,)).fetchone()
except Exception as exc:
    print("UNREADABLE %s: %s" % (type(exc).__name__, exc)); raise SystemExit(1)
if not row:
    print("ABSENT key=%s" % key); raise SystemExit(1)
try:
    wrapper = json.loads(row[0])
    body = wrapper["body"]
    digest_ok = hashlib.sha256(body.encode()).hexdigest() == wrapper.get("digest")
    providers = len(json.loads(body))
except Exception as exc:
    print("MALFORMED %s: %s" % (type(exc).__name__, exc)); raise SystemExit(1)
stamp = datetime.datetime.fromtimestamp((row[1] or 0) / 1000).strftime("%Y-%m-%d %H:%M")
print("%s providers=%d digest=%s bytes=%d updated=%s" % (
    "OK" if digest_ok else "DIGEST-MISMATCH", providers,
    "ok" if digest_ok else "bad", len(body), stamp))
if not digest_ok or providers < min_providers:
    raise SystemExit(1)
raise SystemExit(0)
PY
}

# --- the last known good ---------------------------------------------------------
# Two artifacts, both taken only while the service is healthy: the roster the operator
# actually saw, and the exact catalogue row behind it. They are what a repair restores and
# what the Maestro guard trusts when the live call comes back empty.
take_snapshot() {
  local roster_file="$1" free="$2"
  mkdir -p "$GUARD_DIR"
  python3 - "$DB_PATH" "$CATALOG_KEY" "$CATALOG_SNAPSHOT" <<'PY' 2>/dev/null || warn "opencode-service-guard: could not snapshot the catalogue row"
import json, sqlite3, sys
db, key, dest = sys.argv[1], sys.argv[2], sys.argv[3]
con = sqlite3.connect("file:%s?mode=ro" % db, uri=True, timeout=3)
row = con.execute("select value from kv where key=?", (key,)).fetchone()
if not row:
    raise SystemExit(1)
open(dest, "w").write(row[0])
PY
  cp "$roster_file" "$ROSTER_SNAPSHOT" 2>/dev/null || true
  {
    printf '{"takenAt":"%s","free":%s,"total":%s,"service":"%s","catalog":"%s"}\n' \
      "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "${free:-0}" "$(grep -c . "$roster_file" 2>/dev/null || echo 0)" \
      "$(service_state)" "$(catalog_state)"
  } > "$META_SNAPSHOT"
  say "snapshot: $ROSTER_SNAPSHOT ($free opencode id(s))"
}

# Put a snapshot row back. Used only when the live row is unusable, and only by --repair.
restore_catalog() {
  [ -f "$CATALOG_SNAPSHOT" ] || { warn "opencode-service-guard: no catalogue snapshot at $CATALOG_SNAPSHOT"; return 1; }
  python3 - "$DB_PATH" "$CATALOG_KEY" "$CATALOG_SNAPSHOT" <<'PY'
import hashlib, json, sqlite3, sys, time
db, key, src = sys.argv[1], sys.argv[2], sys.argv[3]
wrapper = json.loads(open(src).read())
body = wrapper["body"]
if hashlib.sha256(body.encode()).hexdigest() != wrapper.get("digest"):
    print("refusing: snapshot digest does not match its body", file=sys.stderr)
    raise SystemExit(1)
if len(json.loads(body)) < 1:
    print("refusing: snapshot body has no providers", file=sys.stderr)
    raise SystemExit(1)
now = int(time.time() * 1000)
wrapper["updatedAt"] = now
con = sqlite3.connect(db, timeout=5)
con.execute("insert into kv(key, value, time_created, time_updated) values(?,?,?,?) "
            "on conflict(key) do update set value=excluded.value, time_updated=excluded.time_updated",
            (key, json.dumps(wrapper), now, now))
con.commit()
print("restored %d provider(s) from snapshot" % len(json.loads(body)))
PY
}

notify() {
  [ "$NOTIFY" = "1" ] || return 0
  osascript -e "display notification \"$(printf '%s' "$1" | sed 's/"/\\"/g')\" with title \"OpenCode model list\"" >/dev/null 2>&1 || true
}

# Re-snapshot only when it would add information: no snapshot yet, a different roster than
# the one on file, or a snapshot older than a day. A 5-minutely job must not rewrite a 5 MB
# catalogue 288 times a day.
snapshot_due() {
  local roster_file="$1" age
  [ -f "$ROSTER_SNAPSHOT" ] || return 0
  [ -f "$CATALOG_SNAPSHOT" ] || return 0
  cmp -s "$roster_file" "$ROSTER_SNAPSHOT" || return 0
  age=$(( $(date +%s) - $(stat -f %m "$CATALOG_SNAPSHOT" 2>/dev/null || echo 0) ))
  [ "$age" -gt 86400 ] && return 0
  return 1
}

# Is the service actually up (not `down`, not just a pid that is gone)?
service_is_up() {
  case "$(service_state)" in
    up*) return 0 ;;
    *)   return 1 ;;
  esac
}

# A freshly started daemon serves an EMPTY roster for its first seconds — measured on v2.0.26:
# 0 ids at t+2s, 18 lines at t+5s, the full roster by t+17s. Checking health the instant a start
# returns would therefore report "free lane missing" after every successful heal, so wait for the
# daemon to be able to answer before judging it.
wait_for_free_lane() {
  local bin="$1" roster_file="$2" limit="${OPENCODE_GUARD_WARMUP:-30}"
  local waited=0 free=0
  while [ "$waited" -lt "$limit" ]; do
    free="$(fetch_roster "$bin" "$roster_file")"
    if [ "${free:-0}" -ge "$MIN_FREE" ]; then
      say "warm    : the free lane answered after ${waited}s"
      return 0
    fi
    sleep 2
    waited=$((waited + 2))
  done
  warn "opencode-service-guard: the service started but served no free lane within ${limit}s"
  return 1
}

# A STOPPED service is the one failure nothing can interrupt: the daemon is already gone, so
# every consumer (`opencode models`, the Maestro picker, a session that has not started yet) is
# already broken, and starting it cannot break anything that was working. That is why this runs
# in --check (the 5-minute timer) while restarting a LIVE daemon stays manual in --repair.
# Bounded with run_bounded() rather than a bare call, so a wedged start cannot hang the timer.
maybe_start_service() {
  local bin="$1" roster_file="$2" timeout="${OPENCODE_GUARD_START_TIMEOUT:-25}"
  if [ "${OPENCODE_GUARD_AUTOSTART:-1}" != "1" ]; then
    warn "opencode-service-guard: service is not up (autostart disabled)"
    return 0
  fi
  say "opencode-service-guard: service is not up — starting it"
  if run_bounded "$timeout" "$bin" service start >/dev/null 2>&1; then
    sleep 2
    if service_is_up; then
      say "started : $(service_state)"
      if wait_for_free_lane "$bin" "$roster_file"; then
        notify "opencode service was down and has been started"
      else
        notify "opencode service started but its free lane did not come back"
      fi
    else
      warn "opencode-service-guard: `service start` returned but the service is still not up"
      notify "opencode service will not start — opencode models is empty for every consumer"
    fi
  else
    warn "opencode-service-guard: `service start` failed or timed out after ${timeout}s"
    warn "  logs: $HOME/.local/share/opencode/log/opencode.log"
    notify "opencode service would not start — the model picker will be blank"
  fi
}

# The health question, in the order that names the cause: the service, then the catalogue
# row it persisted, then the roster it served. Sets HEALTHY and HEALTH_REASON.
check_health() {
  local bin="$1" roster_file="$2"
  local free catalog svc crc total
  catalog="$(catalog_state)"; crc=$?
  svc="$(service_state)"
  free="$(fetch_roster "$bin" "$roster_file")"
  total="$(grep -c . "$roster_file" 2>/dev/null || echo 0)"
  HEALTHY=1
  HEALTH_REASON=""
  say "service : $svc"
  say "catalog : $catalog"
  say "roster  : ${free:-0} opencode id(s) of $total listed"
  if [ "$crc" -ne 0 ]; then
    warn "opencode-service-guard: catalogue row unusable — $catalog"
    HEALTHY=0; HEALTH_REASON="catalogue row unusable"
  fi
  if [ "${free:-0}" -lt "$MIN_FREE" ]; then
    warn "opencode-service-guard: ${free:-0} opencode/* model(s) — the free lane is missing (this is the"
    warn "  empty-list failure: the command still exits 0, so nothing downstream can tell)"
    HEALTHY=0; HEALTH_REASON="${HEALTH_REASON:+$HEALTH_REASON; }free lane missing"
  fi
  case "$svc" in
    stale*) warn "opencode-service-guard: service.json names a pid that is gone ($svc)"
            HEALTHY=0; HEALTH_REASON="${HEALTH_REASON:+$HEALTH_REASON; }stale service record" ;;
  esac
  [ "$HEALTHY" = "1" ] || return 1
  say "healthy : catalogue and free lane both resolve"
  return 0
}

# The only healing action that changes anything a live session holds: a service restart.
# Deliberate, manual, bounded — never on a timer.
repair() {
  local bin="$1" roster_file="$2"
  say "opencode-service-guard repair"
  local before_ok=1
  check_health "$bin" "$roster_file" || before_ok=0
  if [ "$before_ok" = "1" ] && [ "$FORCE" != "1" ]; then
    say "already healthy — nothing to restart (--force restarts anyway)"
    return 0
  fi
  if ! catalog_state >/dev/null 2>&1; then
    warn "opencode-service-guard: catalogue row is unusable — restoring $CATALOG_SNAPSHOT"
    restore_catalog || warn "opencode-service-guard: no usable snapshot to restore (run --snapshot while healthy)"
  fi
  say "restarting the background service: $bin service restart"
  if ! run_bounded 90 "$bin" service restart >/dev/null 2>&1; then
    warn "opencode-service-guard: 'service restart' did not return cleanly within 90s"
  fi
  # A restarted service republishes service.json; give it a moment before judging it.
  local tries=0
  while [ "$tries" -lt 10 ]; do
    sleep 1
    if [ "$(fetch_roster "$bin" "$roster_file" || echo 0)" -ge "$MIN_FREE" ] 2>/dev/null; then break; fi
    tries=$((tries + 1))
  done
  say "after restart:"
  if check_health "$bin" "$roster_file"; then
    snapshot_due "$roster_file" && take_snapshot "$roster_file" "$(grep -c '^opencode/' "$roster_file" 2>/dev/null || echo 0)"
    say "repaired"
    return 0
  fi
  warn "opencode-service-guard: still unhealthy after a restart ($HEALTH_REASON)."
  warn "  The catalogue row is local, so a remaining failure is the served free lane or the"
  warn "  network: check 'opencode service status', then retry; the failed run is in"
  warn "  $LOG_DIR/opencode-service-guard.err.log"
  return 1
}

# --- the interval agent -----------------------------------------------------
# Notify-only on purpose. The one action that heals this is a service restart, and a timer
# that restarts a server out from under a running session is a worse defect than the empty
# list it would fix. The agent's job is to make the failure LOUD within five minutes
# instead of two hours later.
install_agent() {
  local repo template
  repo="$(cd "$(dirname "$0")/.." && pwd)"
  template="$repo/scripts/$PLIST_LABEL.plist.template"
  [ -f "$template" ] || die "template missing: $template"
  mkdir -p "$LOG_DIR" "$(dirname "$PLIST_DEST")"
  sed -e "s#{{REPO_ROOT}}#$repo#g" \
      -e "s#{{HOME}}#$HOME#g" \
      -e "s#{{LOG_DIR}}#$LOG_DIR#g" \
      -e "s#{{LABEL}}#$PLIST_LABEL#g" \
      -e "s#{{PATH}}#$PATH#g" \
      "$template" > "$PLIST_DEST"
  plutil -lint "$PLIST_DEST" >/dev/null || die "generated plist does not lint: $PLIST_DEST"
  launchctl bootout "gui/$(id -u)/$PLIST_LABEL" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$PLIST_DEST"
  launchctl enable "gui/$(id -u)/$PLIST_LABEL" 2>/dev/null || true
  say "installed: $PLIST_DEST"
  say "  runs  : $repo/scripts/opencode-service-guard.sh --check --quiet --notify (at login, then every 300s)"
  say "  heals : nothing on its own — it notifies, so a live session is never interrupted"
  say "  fix   : pnpm opencode:guard:repair  (restore catalogue row + restart the service)"
  say "  logs  : $LOG_DIR/opencode-service-guard.log"
}

uninstall_agent() {
  launchctl bootout "gui/$(id -u)/$PLIST_LABEL" 2>/dev/null || true
  rm -f "$PLIST_DEST"
  say "uninstalled: $PLIST_LABEL"
}

# --- main -------------------------------------------------------------------
main() {
  case "$MODE" in
    install) install_agent; exit $? ;;
    uninstall) uninstall_agent; exit $? ;;
  esac

  local bin roster_file free
  bin="$(resolve_opencode_bin || true)"
  [ -n "$bin" ] || die "no opencode CLI found on PATH (set OPENCODE_BIN)"
  roster_file="$(mktemp -t opencode-service-guard-roster)"
  trap 'rm -f "${roster_file:-}"' EXIT

  case "$MODE" in
    status)
      say "opencode-service-guard status"
      free="$(fetch_roster "$bin" "$roster_file")"
      report_status "$bin" "$free" "$roster_file"
      return 0
      ;;
    snapshot)
      if check_health "$bin" "$roster_file" >/dev/null 2>&1; then
        free="$(grep -c '^opencode/' "$roster_file" 2>/dev/null || echo 0)"
        take_snapshot "$roster_file" "$free"
        return 0
      fi
      warn "opencode-service-guard: refusing to snapshot an unhealthy state"
      check_health "$bin" "$roster_file"
      return 1
      ;;
    repair)
      repair "$bin" "$roster_file"
      return $?
      ;;
  esac

  # default: --check
  service_is_up || maybe_start_service "$bin" "$roster_file"

  if check_health "$bin" "$roster_file"; then
    if snapshot_due "$roster_file"; then
      free="$(grep -c '^opencode/' "$roster_file" 2>/dev/null || echo 0)"
      take_snapshot "$roster_file" "$free"
    fi
    return 0
  fi
  notify "opencode models is unhealthy: $HEALTH_REASON"
  warn "opencode-service-guard: unhealthy — $HEALTH_REASON"
  warn "  heal with: pnpm opencode:guard:repair"
  return 1
}

report_status() {
  local bin="$1" free="$2" roster_file="$3"
  printf '  opencode CLI   : %s\n' "${bin:-<not found>}"
  [ -n "$bin" ] && printf '  version        : %s\n' "$("$bin" --version 2>/dev/null || echo unknown)"
  printf '  service        : %s\n' "$(service_state)"
  printf '  catalogue row  : %s\n' "$(catalog_state)"
  printf '  live roster    : %s opencode id(s) of %s listed\n' "$free" "$(grep -c . "$roster_file" 2>/dev/null || echo 0)"
  if [ -f "$META_SNAPSHOT" ]; then
    printf '  last good      : %s\n' "$(cat "$META_SNAPSHOT")"
    printf '  snapshot age   : %s min\n' "$(( ( $(date +%s) - $(stat -f %m "$META_SNAPSHOT") ) / 60 ))"
  else
    printf '  last good      : none yet (run --snapshot while healthy)\n'
  fi
  if launchctl print "gui/$(id -u)/$PLIST_LABEL" >/dev/null 2>&1; then
    printf '  interval agent : loaded (%s, every 5 min, notify only)\n' "$PLIST_LABEL"
  else
    printf '  interval agent : not loaded (run --install)\n'
  fi
}

main




