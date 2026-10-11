#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/maestro-model-guard.sh — keep every SAVED model selection pointing at a
# LIVE OpenCode model, and make the authoritative OpenCode config un-overwritable.
#
# WHY THIS EXISTS (measured 2026-10-10)
# Maestro is a CONSUMER of the OpenCode harness, never its author. It persists the model
# name each chat tab last used (`turnModel`, `customModel`) in
#   ~/Library/Application Support/Maestro/maestro-sessions.json
# and restores it blindly on resume. When the provider retires a model — it retired
# `opencode/fledge-alpha-free` on 2026-10-10, after retiring several before it — every
# restored tab re-selects a dead id, the vendor answers
#   SessionRunnerModel.ModelUnavailableError: Model unavailable: opencode/fledge-alpha-free
# and the picker reads as "my OpenCode model list is broken again". It was the sixth such
# break in two weeks, and the cause was always a stale pin, never a corrupt catalogue.
#
# Nothing on the OpenCode or Forge side can stop a GUI from persisting its own state, so
# this guard repairs that state instead, fails closed when it cannot trust the live
# roster, and can run at login before Maestro is opened.
#
# MODES
#   --check      report only; exit 1 when a stale pin is present (safe in a gate)
#   --repair     (DEFAULT) rewrite stale pins to their live successor; timestamped backups first
#   --lock       make ~/.config/opencode/opencode.json immutable (chflags uchg) so NO process
#                may overwrite your provider/model definitions; a golden copy is kept first
#   --unlock     revert --lock
#   --install    install a login LaunchAgent that repairs before Maestro is opened
#   --uninstall  remove that LaunchAgent
#   --status     print roster size, stale-pin count, lock state and agent state
#   --set-default write configs.opencode.model — which Maestro ships as "" — to an id the LIVE
#                roster actually serves, so the opencode lane stops falling back to whatever a
#                tab last used. Only ever an id from the roster this run read.
#
# FLAGS
#   --force      repair while Maestro's window is open, and remap UNKNOWN dead `opencode/*`
#                ids to $MAESTRO_MODEL_FALLBACK instead of only reporting them
#   --quiet      print problems only
#
# EXIT: 0 clean/repaired · 1 stale found under --check · 2 refused (bad flags, missing tools,
#       untrusted roster, or Maestro open during --repair without --force)
# ---------------------------------------------------------------------------

set -uo pipefail

MAESTRO_DIR="${MAESTRO_DIR:-$HOME/Library/Application Support/Maestro}"
SESSIONS="$MAESTRO_DIR/maestro-sessions.json"
AGENT_CONFIGS="$MAESTRO_DIR/maestro-agent-configs.json"
OPENCODE_CONFIG_DIR="${OPENCODE_CONFIG_DIR:-$HOME/.config/opencode}"
OPENCODE_CONFIG="$OPENCODE_CONFIG_DIR/opencode.json"
STATE_MODEL_JSON="${STATE_MODEL_JSON:-$HOME/.local/state/opencode/model.json}"
OPENCODE_BIN="${OPENCODE_BIN:-}"
FALLBACK_MODEL="${MAESTRO_MODEL_FALLBACK:-opencode/step-5-preview-free}"
LOG_DIR="${AGENT_WORKER_LOG_DIR:-/Users/Shared/dev/build/logs}"
PLIST_LABEL="com.culebraluxe.maestro-model-guard"
PLIST_DEST="$HOME/Library/LaunchAgents/$PLIST_LABEL.plist"

# Retired model id -> the live model that replaced it. Extend this table when a model retires:
# that single line is what a future break needs, instead of a seventh lost afternoon.
RENAME_PAIRS="${MAESTRO_MODEL_RENAMES:-opencode/fledge-alpha-free=opencode/step-5-preview-free}"

# A roster this small cannot be trusted. `opencode models` fails OPEN — the background service
# that answers it can return nothing and exit 0 — so an empty roster would read as "every model
# is dead" and this script would rewrite live pins into nonsense. Below the floor we refuse to
# judge anything we were not explicitly told to rename, and fall back to the last known good
# snapshot (ROSTER_FALLBACK) when one exists.
MIN_ROSTER="${MAESTRO_MODEL_MIN_ROSTER:-5}"

# The roster the OpenCode service guard snapshotted while the service was healthy. Used only
# when the live call comes back short, so a broken OpenCode service cannot freeze pin repair
# (measured 2026-10-11; see scripts/opencode-service-guard.sh for why the call can be empty).
ROSTER_FALLBACK="${MAESTRO_MODEL_ROSTER_FALLBACK:-$HOME/.local/state/opencode/guard/roster.txt}"

# What an EMPTY lane model should become. Empty means "pick one from the live roster at run
# time" (resolve_default_model) — Maestro ships `configs.opencode.model` as "".
DEFAULT_MODEL="${MAESTRO_MODEL_DEFAULT:-}"

MODE=repair
QUIET=0
FORCE=0

usage() {
  cat <<'USAGE'
usage: scripts/maestro-model-guard.sh [--check|--repair|--lock|--unlock|--install|--uninstall|--status|--set-default]
                                      [--force] [--quiet]

  --set-default  write configs.opencode.model (Maestro ships it empty, which makes the opencode
                 lane fall back to whatever a tab last used) to a model that is in the LIVE
                 roster: $MAESTRO_MODEL_DEFAULT when set, else the first served opencode/*-free id
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --check) MODE=check ;;
    --repair) MODE=repair ;;
    --lock) MODE=lock ;;
    --unlock) MODE=unlock ;;
    --install) MODE=install ;;
    --uninstall) MODE=uninstall ;;
    --status) MODE=status ;;
    --set-default) MODE=set-default ;;
    --force) FORCE=1 ;;
    --quiet|-q) QUIET=1 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "maestro-model-guard: unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

say() { [ "$QUIET" = "1" ] || printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }
die() { warn "maestro-model-guard: $*"; exit 2; }

# The vendor CLI, by precedence: an explicit override, the vendor home layout, then PATH.
resolve_opencode_bin() {
  if [ -n "$OPENCODE_BIN" ] && [ -x "$OPENCODE_BIN" ]; then printf '%s' "$OPENCODE_BIN"; return 0; fi
  if [ -x "$HOME/.opencode/bin/opencode" ]; then printf '%s' "$HOME/.opencode/bin/opencode"; return 0; fi
  local on_path
  on_path="$(command -v opencode 2>/dev/null || true)"
  if [ -n "$on_path" ]; then printf '%s' "$on_path"; return 0; fi
  return 1
}

# TRUE only for Maestro's own window process. The orphaned coworking MCP helpers share the
# process name, so matching on the name alone would refuse forever.
maestro_window_open() {
  pgrep -f '^/Applications/Maestro\.app/Contents/MacOS/Maestro$' >/dev/null 2>&1
}

# The live roster, captured to a file so the JSON pass never re-dials the network.
#
# FAIL-SAFE (2026-10-11): the background service behind `opencode models` can return NOTHING and
# exit 0 (measurement in scripts/opencode-service-guard.sh). An empty roster is not "no models
# exist", it is "the service did not answer" — and treating it as the former would either freeze
# every repair or rewrite live pins into nonsense. So when the live call is short of MIN_ROSTER,
# fall back to the last known good roster the service guard snapshotted while it was healthy.
fetch_roster() {
  local bin="$1" out="$2" live count fallback
  live="$("$bin" models 2>/dev/null | grep -E '^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$' | sort -u)"
  printf '%s\n' "$live" | grep -v '^$' > "$out" || true
  count="$(grep -c . "$out" 2>/dev/null || true)"
  count="${count:-0}"
  if [ "$count" -lt "$MIN_ROSTER" ] && [ -f "$ROSTER_FALLBACK" ]; then
    fallback="$(grep -c . "$ROSTER_FALLBACK" 2>/dev/null || true)"
    fallback="${fallback:-0}"
    if [ "$fallback" -ge "$MIN_ROSTER" ]; then
      grep -v '^$' "$ROSTER_FALLBACK" > "$out" || true
      warn "maestro-model-guard: live roster is short ($count) — using the last known good snapshot"
      warn "  ($fallback model(s), saved $(stat -f '%Sm' -t '%Y-%m-%d %H:%M' "$ROSTER_FALLBACK" 2>/dev/null)); heal the service with: pnpm opencode:guard:repair"
      count="$fallback"
    fi
  fi
  printf '%s' "$count"
}

# The model an empty lane default should become: the named fallback when the live roster still
# serves it, else the first served free-lane id, else the first opencode id. Only ids that are
# in the roster this run read — never a guessed name.
resolve_default_model() {
  local roster_file="$1" pick
  if [ -n "$DEFAULT_MODEL" ]; then printf '%s' "$DEFAULT_MODEL"; return 0; fi
  pick="$(grep -x -F "$FALLBACK_MODEL" "$roster_file" 2>/dev/null | head -1 || true)"
  [ -n "$pick" ] && { printf '%s' "$pick"; return 0; }
  pick="$(grep -E '^opencode/[A-Za-z0-9._-]+-free$' "$roster_file" 2>/dev/null | head -1 || true)"
  [ -n "$pick" ] && { printf '%s' "$pick"; return 0; }
  pick="$(grep -E '^opencode/' "$roster_file" 2>/dev/null | head -1 || true)"
  [ -n "$pick" ] && { printf '%s' "$pick"; return 0; }
  return 1
}

# Write configs.opencode.model under a timestamped backup. This is the one Maestro value this
# guard AUTHORS, and only for the lane it owns: the model has to come from the live roster, so a
# service outage can never write a dead id into it.
set_default_pass() {
  local model="$1"
  AGENT_CONFIGS="$AGENT_CONFIGS" MODEL="$model" \
  python3 - <<'PY'
import json, os, time
path, model = os.environ["AGENT_CONFIGS"], os.environ["MODEL"]
text = open(path).read()
data = json.loads(text)
cfg = data.setdefault("configs", {}).setdefault("opencode", {})
before = cfg.get("model")
if before == model:
    print("configs.opencode.model: already %s" % model)
    raise SystemExit(0)
stamp = time.strftime("%Y%m%d-%H%M%S")
with open("%s.bak-modelguard-%s" % (path, stamp), "wb") as fh:
    fh.write(text.encode())
cfg["model"] = model
indent = "\t" if "\n\t" in text else 2
with open(path, "w") as fh:
    fh.write(json.dumps(data, indent=indent, ensure_ascii=False) + "\n")
json.load(open(path))
print("configs.opencode.model: %s -> %s" % (before or '""', model))
print("  backup: %s.bak-modelguard-%s" % (path, stamp))
PY
}

# One pass over every stored model selection. `--check` only reports; `--repair` rewrites
# exact-match model fields under a timestamped backup. Chat text is never touched: the walk
# descends into chat logs but only ever rewrites the value of a model-named key.
repair_pass() {
  local roster_file="$1"
  SESSIONS="$SESSIONS" AGENT_CONFIGS="$AGENT_CONFIGS" STATE_MODEL_JSON="$STATE_MODEL_JSON" \
  MODE="$MODE" QUIET="$QUIET" FORCE="$FORCE" FALLBACK_MODEL="$FALLBACK_MODEL" \
  ROSTER_FILE="$roster_file" RENAME_PAIRS="$RENAME_PAIRS" MIN_ROSTER="$MIN_ROSTER" \
  DEFAULT_MODEL="${DEFAULT_MODEL_RESOLVED:-}" \
  python3 - <<'PY'
import json, os, sys, time

env = os.environ.get
sessions_p, agent_p, state_p = env("SESSIONS"), env("AGENT_CONFIGS"), env("STATE_MODEL_JSON")
mode, quiet, force = env("MODE", "repair"), env("QUIET") == "1", env("FORCE") == "1"
fallback, roster_file = env("FALLBACK_MODEL"), env("ROSTER_FILE")
default_model = env("DEFAULT_MODEL", "")
min_roster = int(env("MIN_ROSTER", "5"))

roster = set()
if roster_file and os.path.exists(roster_file):
    with open(roster_file) as fh:
        roster = {line.strip() for line in fh if line.strip()}
roster_trusted = len(roster) >= min_roster

renames = {}
for pair in env("RENAME_PAIRS", "").split(","):
    if "=" in pair:
        old, new = pair.split("=", 1)
        renames[old.strip()] = new.strip()

MODEL_KEYS = {"turnModel", "customModel", "model"}
stale, unknown, fixed, dropped = [], [], [], []

def heal_model(path, value):
    if not isinstance(value, str) or not value.strip():
        return value
    if value in renames:
        stale.append((path, value))
        fixed.append((path, value, renames[value]))
        return renames[value]
    if value.startswith("opencode/") and roster_trusted and value not in roster:
        # An unseen dead id. Only remap when explicitly forced: guessing a replacement for a
        # name nobody has mapped yet is how a guard becomes the next outage.
        if force:
            stale.append((path, value))
            fixed.append((path, value, fallback))
            return fallback
        unknown.append((path, value))
    return value

def walk(node, path):
    if isinstance(node, dict):
        for key, value in list(node.items()):
            child = path + "." + key if path else key
            if key in MODEL_KEYS:
                if isinstance(value, str) and value.strip():
                    healed = heal_model(child, value)
                    if healed != value:
                        node[key] = healed
                    continue
                if child == "configs.opencode.model":
                    # Maestro ships this lane's default as "". An empty default means the app
                    # falls back to whatever the tab last used — the dead pin this guard exists
                    # to catch — so an empty one is a finding, not "nothing to do".
                    stale.append((child, value if isinstance(value, str) else ""))
                    if mode == "repair" and default_model and roster_trusted:
                        node[key] = default_model
                        fixed.append((child, "", default_model))
                    continue
            walk(value, child)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            walk(value, "%s[%d]" % (path, index))

def heal_state(node, path):
    """Drop dead ids from the operator's remembered model history; never invent entries."""
    if not roster_trusted:
        return
    for key in ("recent", "favorite"):
        entries = node.get(key)
        if not isinstance(entries, list):
            continue
        kept = []
        for entry in entries:
            full = None
            if isinstance(entry, dict) and entry.get("providerID") and entry.get("modelID"):
                full = "%s/%s" % (entry["providerID"], entry["modelID"])
            if full and full not in roster:
                dropped.append(("%s.%s" % (path, key), full))
                continue
            kept.append(entry)
        node[key] = kept
    variant = node.get("variant")
    if isinstance(variant, dict):
        for full in [key for key in variant if key not in roster]:
            dropped.append(("%s.variant" % path, full))
            variant.pop(full, None)

def apply(path, mutate, label):
    with open(path) as fh:
        text = fh.read()
    try:
        data = json.loads(text)
    except ValueError:
        return False
    before = len(stale) + len(unknown) + len(dropped)
    mutate(data, label)
    if len(stale) + len(unknown) + len(dropped) == before:
        return False
    indent = "\t" if "\n\t" in text else 2
    rendered = json.dumps(data, indent=indent, ensure_ascii=False) + "\n"
    if rendered == text:
        return False
    if mode == "repair":
        stamp = time.strftime("%Y%m%d-%H%M%S")
        with open(path, "rb") as src, open("%s.bak-modelguard-%s" % (path, stamp), "wb") as dst:
            dst.write(src.read())
        with open(path, "w") as fh:
            fh.write(rendered)
    return True

written = []
for target in [p for p in (sessions_p, agent_p) if p and os.path.exists(p)]:
    if apply(target, lambda data, _label: walk(data, ""), ""):
        written.append(target)
if state_p and os.path.exists(state_p):
    if apply(state_p, heal_state, "model.json"):
        written.append(state_p)

if not quiet:
    print("roster: %d live model(s)%s" % (
        len(roster), "" if roster_trusted else " — UNTRUSTED, only named renames are applied"))
    fixed_label = "fixed" if mode == "repair" else "stale"
    for path, old, new in fixed:
        print("  %-7s %s  %s -> %s" % (fixed_label, path, old, new))
    for path, value in unknown:
        print("  unknown %s  %s  (no rename known; --force remaps to %s)" % (path, value, fallback))
    for path, value in dropped:
        print("  dropped %s  %s  (dead id removed from remembered history)" % (path, value))
    for path in written:
        print("  %s %s" % ("wrote  " if mode == "repair" else "stale  ", path))

if unknown:
    print("maestro-model-guard: %d unknown dead id(s) need a rename row" % len(unknown))
    sys.exit(1)
if mode == "check" and stale:
    print("maestro-model-guard: %d stale pin(s) found" % len(stale))
    sys.exit(1)
if mode == "repair" and fixed:
    print("maestro-model-guard: repaired %d pin(s) in %d file(s)" % (len(fixed), len(written)))
sys.exit(0)
PY
}

# --- the config lock --------------------------------------------------------
# The one file a consumer must never author: our provider/model definitions. `uchg` makes the
# kernel refuse every write, so a future overwrite fails loudly (EPERM) instead of silently
# replacing a working catalogue. The golden copy is what you diff against when it does.
lock_config() {
  [ -f "$OPENCODE_CONFIG" ] || die "no config to lock: $OPENCODE_CONFIG"
  local jsonc="$OPENCODE_CONFIG_DIR/opencode.jsonc"
  if [ ! -f "$OPENCODE_CONFIG.golden" ]; then
    cp -p "$OPENCODE_CONFIG" "$OPENCODE_CONFIG.golden"
    say "golden copy: $OPENCODE_CONFIG.golden"
  fi
  chflags uchg "$OPENCODE_CONFIG" || die "chflags uchg failed on $OPENCODE_CONFIG"
  [ -f "$jsonc" ] && chflags uchg "$jsonc"
  # Redirect the probe's stderr FIRST: a failing output redirect is reported by the shell
  # before a trailing 2>/dev/null takes effect, so the intended EPERM failure would leak
  # into the login agent's error log on every run.
  if printf '' 2>/dev/null >> "$OPENCODE_CONFIG"; then
    warn "maestro-model-guard: the flag did not take — $OPENCODE_CONFIG is still writable"
    return 1
  fi
  say "locked (uchg): $OPENCODE_CONFIG"
  [ -f "$jsonc" ] && say "locked (uchg): $jsonc"
  say "revert with: scripts/maestro-model-guard.sh --unlock"
  return 0
}

unlock_config() {
  local jsonc="$OPENCODE_CONFIG_DIR/opencode.jsonc"
  chflags nouchg "$OPENCODE_CONFIG" 2>/dev/null || true
  [ -f "$jsonc" ] && chflags nouchg "$jsonc" 2>/dev/null
  say "unlocked: $OPENCODE_CONFIG"
  return 0
}

# --- the login agent --------------------------------------------------------
install_agent() {
  local repo template
  repo="$(cd "$(dirname "$0")/.." && pwd)"
  template="$repo/scripts/$PLIST_LABEL.plist.template"
  [ -f "$template" ] || die "template missing: $template"
  mkdir -p "$HOME/Library/LaunchAgents" "$LOG_DIR"
  # The repo lives outside ~/Documents, so launchd may execute this script in place — no
  # deployed copy is needed, unlike the agent worker's wrapper.
  sed -e "s#{{REPO_ROOT}}#$repo#g" \
      -e "s#{{HOME}}#$HOME#g" \
      -e "s#{{LOG_DIR}}#$LOG_DIR#g" \
      -e "s#{{LABEL}}#$PLIST_LABEL#g" \
      -e "s#{{PATH}}#$PATH#g" \
      "$template" > "$PLIST_DEST"
  plutil -lint "$PLIST_DEST" >/dev/null
  launchctl bootout "gui/$(id -u)/$PLIST_LABEL" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$PLIST_DEST"
  launchctl enable "gui/$(id -u)/$PLIST_LABEL" 2>/dev/null || true
  say "installed: $PLIST_DEST"
  say "  runs  : $repo/scripts/maestro-model-guard.sh --repair --quiet (at login)"
  say "  logs  : $LOG_DIR/maestro-model-guard.log"
}

uninstall_agent() {
  launchctl bootout "gui/$(id -u)/$PLIST_LABEL" 2>/dev/null || true
  rm -f "$PLIST_DEST"
  say "uninstalled: $PLIST_LABEL"
}

# --- status -----------------------------------------------------------------
report_status() {
  local bin="$1" roster_count="$2" jsonc="$OPENCODE_CONFIG_DIR/opencode.jsonc"
  printf '  opencode CLI   : %s\n' "${bin:-<not found>}"
  [ -n "$bin" ] && printf '  version        : %s\n' "$("$bin" --version 2>/dev/null || echo unknown)"
  printf '  live roster    : %s model(s)\n' "$roster_count"
  if [ -f "$ROSTER_FALLBACK" ]; then
    printf '  roster base    : last good snapshot %s (%s model(s))\n' \
      "$(stat -f '%Sm' -t '%Y-%m-%d %H:%M' "$ROSTER_FALLBACK")" \
      "$(grep -c . "$ROSTER_FALLBACK" 2>/dev/null || echo 0)"
  else
    printf '  roster base    : no service-guard snapshot yet (pnpm opencode:guard:snapshot)\n'
  fi
  printf '  opencode lane  : %s\n' \
    "$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["configs"]["opencode"].get("model") or "<empty — falls back to the last used id>")' 2>/dev/null "$AGENT_CONFIGS" || echo 'unreadable')"
  printf '  sessions file  : %s\n' "$([ -f "$SESSIONS" ] && stat -f '%z bytes, modified %Sm' -t '%Y-%m-%d %H:%M' "$SESSIONS" || echo missing)"
  if ls -lO "$OPENCODE_CONFIG" 2>/dev/null | grep -q uchg; then
    printf '  config lock    : LOCKED (uchg) %s\n' "$OPENCODE_CONFIG"
  elif [ -f "$OPENCODE_CONFIG" ]; then
    printf '  config lock    : writable %s  (run --lock to make it un-overwritable)\n' "$OPENCODE_CONFIG"
  else
    printf '  config lock    : no config at %s\n' "$OPENCODE_CONFIG"
  fi
  [ -f "$jsonc" ] && ls -lO "$jsonc" | grep -q uchg && printf '  config lock    : LOCKED (uchg) %s\n' "$jsonc"
  if launchctl print "gui/$(id -u)/$PLIST_LABEL" >/dev/null 2>&1; then
    printf '  login agent    : loaded (%s)\n' "$PLIST_LABEL"
  else
    printf '  login agent    : not loaded (run --install)\n'
  fi
  if maestro_window_open; then
    printf '  Maestro window : OPEN — repairs are refused until it is quit\n'
  else
    printf '  Maestro window : closed\n'
  fi
}

# --- main -------------------------------------------------------------------
main() {
  case "$MODE" in
    lock) lock_config; exit $? ;;
    unlock) unlock_config; exit $? ;;
    install) install_agent; exit $? ;;
    uninstall) uninstall_agent; exit $? ;;
  esac

  local bin roster_file count
  bin="$(resolve_opencode_bin || true)"
  roster_file="$(mktemp -t maestro-model-guard-roster)"
  trap 'rm -f "${roster_file:-}"' EXIT
  : > "$roster_file"
  count=0
  if [ -n "$bin" ]; then
    count="$(fetch_roster "$bin" "$roster_file")"
  else
    warn "maestro-model-guard: no opencode CLI found — applying named renames only (set OPENCODE_BIN)"
  fi
  if [ "$count" -lt "$MIN_ROSTER" ]; then
    warn "maestro-model-guard: roster has only $count live model(s) — untrusted, so only explicitly renamed ids are touched"
  fi

  # The model an empty lane default becomes. Only ever an id the roster above carries.
  DEFAULT_MODEL_RESOLVED=""
  if [ "$count" -ge "$MIN_ROSTER" ]; then
    DEFAULT_MODEL_RESOLVED="$(resolve_default_model "$roster_file" || true)"
  fi

  if [ "$MODE" = "status" ]; then say "maestro-model-guard status"; fi

  if [ "$MODE" = "set-default" ]; then
    [ "$count" -ge "$MIN_ROSTER" ] || die "roster has only $count live model(s) — refusing to write a default from an untrusted roster"
    if maestro_window_open && [ "$FORCE" != "1" ]; then
      die "Maestro's window is open. It holds this config in memory and would overwrite the write on
quit. Quit Maestro (Cmd-Q) and re-run — or pass --force."
    fi
    local model
    model="$(resolve_default_model "$roster_file")" || die "could not choose a default: the roster carries no opencode/* id"
    say "setting configs.opencode.model -> $model  (roster: $count live model(s))"
    set_default_pass "$model"
    return $?
  fi

  if [ "$MODE" = "repair" ] && [ "$FORCE" != "1" ] && maestro_window_open; then
    die "Maestro's window is open. It holds this state in memory and would overwrite a repair on
quit. Quit Maestro (Cmd-Q) and re-run — or pass --force to repair anyway."
  fi

  repair_pass "$roster_file"
  local rc=$?
  if [ "$MODE" = "status" ]; then
    report_status "$bin" "$count"
    return 0
  fi
  return $rc
}

main
