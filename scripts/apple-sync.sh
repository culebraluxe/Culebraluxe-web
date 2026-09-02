#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — canonical Apple Messages -> PROD Client timeline sync.
#
# THE single command for the normal Apple production cycle. Run it manually
# (./scripts/apple-sync.sh) or from launchd via the FDA-grantable launcher
# (com.culebraluxe.apple-sync). BOTH invoke THIS same script — there is no
# separate scheduled implementation.
#
#   ~/Library/Messages/chat.db
#     -> apple-messages-export (READ-ONLY)
#     -> public/upload/data/apple-messages-export/   (gitignored package)
#     -> scripts/apple-messages-intake.ts prod        (ODS -> reconcile -> interaction)
#     -> Client read-model refresh  (conversation-burst Contact History)
#
# Replay-safe: canonical interactions key on (source_system=apple_messages,
# source_external_id=Apple GUID). Later runs insert new GUIDs and replay the rest
# with zero duplicates.
#
# Lock: a single atomic mkdir lock (/tmp/culebraluxe-apple-sync.lock). If another
# valid sync is running we skip cleanly (exit 0). Stale locks (dead PID / old age)
# are reclaimed. This prevents two ~90k-message PROD jobs racing.
#
# Env (all optional):
#   CULEBRALUXE_REPO             repository root (set by launcher/deployed copy)
#   CULEBRALUXE_APPLE_LOG_DIR    log directory (default ~/Library/Logs/CulebraLuxe)
#   CULEBRALUXE_APPLE_LOG        log file (default $LOG_DIR/apple-sync.log)
#   APPLE_SYNC_LOCK              lock path (default /tmp/culebraluxe-apple-sync.lock)
#   APPLE_SYNC_EXPORTER_DEBUG    set to 1 to use the Swift DEBUG build
# ---------------------------------------------------------------------------
set -uo pipefail

# --- repo root: reliable from launchd (CULEBRALUXE_REPO) OR manual run ---
if [ -n "${CULEBRALUXE_REPO:-}" ]; then
  REPO_ROOT="$CULEBRALUXE_REPO"
else
  SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
  case "$SELF_DIR" in
    */scripts) REPO_ROOT="$(cd "$SELF_DIR/.." && pwd -P)" ;;
    *) REPO_ROOT="$SELF_DIR" ;;
  esac
fi
cd "$REPO_ROOT" || { echo "cannot cd to repo root: $REPO_ROOT" >&2; exit 1; }

LOG_DIR="${CULEBRALUXE_APPLE_LOG_DIR:-$HOME/Library/Logs/CulebraLuxe}"
LOG_FILE="${CULEBRALUXE_APPLE_LOG:-$LOG_DIR/apple-sync.log}"
LOCK="${APPLE_SYNC_LOCK:-/tmp/culebraluxe-apple-sync.lock}"
EXPORT_DIR="$REPO_ROOT/public/upload/data/apple-messages-export"
INTAKE="$REPO_ROOT/scripts/apple-messages-intake.ts"
PKG_DIR="$REPO_ROOT/apple-messages-export"
RUN_CFG="${APPLE_SYNC_EXPORTER_DEBUG:-0}"

mkdir -p "$LOG_DIR" || { echo "cannot create log dir: $LOG_DIR" >&2; exit 1; }

now() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
start_epoch="$(date +%s)"
dur() { echo $(( $(date +%s) - start_epoch )); }

# Log to BOTH the terminal and the durable log file. No message bodies ever.
step_log() { echo "[apple-sync $(now)] $*" | tee -a "$LOG_FILE"; }

finish() {
  local status="$1"; local code="$2"
  echo "===== APPLE SYNC END $(now) status=$status duration_s=$(dur) =====" >>"$LOG_FILE"
  exit "$code"
}
fail() {
  echo "[apple-sync $(now)] ERROR: $*" | tee -a "$LOG_FILE"
  finish FAILED 1
}

# --- single-flight lock ------------------------------------------------------
acquire_lock() {
  if mkdir "$LOCK" 2>/dev/null; then
    echo "$$" > "$LOCK/pid"
    return 0
  fi
  local pid=""
  [ -f "$LOCK/pid" ] && pid="$(cat "$LOCK/pid" 2>/dev/null || true)"
  local stale=no
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    : # live PID -> genuinely running
  else
    stale=yes
  fi
  if [ "$stale" = "no" ] && [ -f "$LOCK/pid" ]; then
    local age=0
    age=$(( $(date +%s) - $(stat -f '%m' "$LOCK/pid" 2>/dev/null || printf '%s' "$(date +%s)") ))
    [ "$age" -gt 14400 ] && stale=yes
  fi
  if [ "$stale" = "yes" ]; then
    rm -rf "$LOCK"
    if mkdir "$LOCK" 2>/dev/null; then
      echo "$$" > "$LOCK/pid"
      return 0
    fi
  fi
  step_log "Apple sync already running; skipping."
  return 1
}

# --- main -------------------------------------------------------------------
{
  echo "===== APPLE SYNC START $(now) ====="
  echo "repo_root=$REPO_ROOT"
  echo "export_dir=$EXPORT_DIR"
  echo "log_file=$LOG_FILE"
} >>"$LOG_FILE"

if ! acquire_lock; then
  finish SKIPPED 0
fi
trap 'rm -rf "$LOCK"' EXIT

step_log "verifying environment"
[ -f .env.local ] || fail "missing .env.local at $REPO_ROOT"
node -e '
const fs=require("fs");
const s=fs.readFileSync(".env.local","utf8");
for (const l of s.split(/\r?\n/)) {
  const t=l.trim();
  if (t.startsWith("DATABASE_URL_PROD=")) { if (t.slice(18).trim()) process.exit(0); }
}
process.exit(2);
' || fail "DATABASE_URL_PROD missing/empty in .env.local (refusing to touch pipeline)"
[ -d node_modules/tsx ] || fail "tsx not installed (run pnpm install)"
command -v swift >/dev/null 2>&1 || fail "swift not found in PATH"
command -v node >/dev/null 2>&1 || fail "node not found in PATH"

# --- fresh export ------------------------------------------------------------
step_log "exporter start"
if [ "$RUN_CFG" = "1" ]; then
  export_cmd=(swift run --package-path "$PKG_DIR" apple-messages-export --out "$EXPORT_DIR")
else
  export_cmd=(swift run -c release --package-path "$PKG_DIR" apple-messages-export --out "$EXPORT_DIR")
fi
export_out="$(mktemp)"; export_err="$(mktemp)"
if "${export_cmd[@]}" >"$export_out" 2>"$export_err"; then
  step_log "exporter success"
  cat "$export_out" >>"$LOG_FILE"
  handles="$(grep -Eo '^handles=[0-9]+' "$export_out" | tail -1 | tr -dc '0-9')"
  messages="$(grep -Eo ' messages=[0-9]+' "$export_out" | tail -1 | tr -dc '0-9')"
  minmax="$(grep -E '^min=' "$export_out" | tail -1)"
  step_log "exported handles=${handles:-?} messages=${messages:-?} ${minmax:-}"
else
  step_log "exporter FAILED"
  tail -40 "$export_err" | tee -a "$LOG_FILE"
  rm -f "$export_out" "$export_err"
  fail "apple-messages-export failed; NO PROD intake run (export package preserved)"
fi
rm -f "$export_out" "$export_err"

# --- validate the export package ----------------------------------------------
step_log "validating export package"
[ -f "$EXPORT_DIR/manifest.json" ] || fail "manifest.json missing; NO PROD intake run"
[ -f "$EXPORT_DIR/identities.jsonl" ] || fail "identities.jsonl missing; NO PROD intake run"
[ -f "$EXPORT_DIR/messages.jsonl" ] || fail "messages.jsonl missing; NO PROD intake run"
msg_count="$(wc -l < "$EXPORT_DIR/messages.jsonl" 2>/dev/null | tr -d ' ')"
[ -n "$msg_count" ] && [ "$msg_count" -gt 0 ] || fail "messages.jsonl is empty; NO PROD intake run"
manifest_dated="$(node -e 'const m=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));console.log(m.minimumMessageDate!=null&&m.minimumMessageDate!==""?"1":"0")' "$EXPORT_DIR/manifest.json" 2>/dev/null || echo "0")"
[ "$manifest_dated" = "1" ] || fail "no dated messages (manifest.minimumMessageDate null); NO PROD intake run"
step_log "validation OK: messages=$msg_count dated=yes"

# --- PROD intake + replay idempotency proof -----------------------------------
step_log "intake start (PROD)"
intake_out="$(mktemp)"; intake_err="$(mktemp)"
# Stream intake progress to the operator while retaining copies for the final
# tally and failure diagnostics. Process substitution preserves the node exit
# status as the condition for this if statement.
if node --env-file=.env.local --import tsx "$INTAKE" prod --dir "$EXPORT_DIR" \
  > >(tee "$intake_out") \
  2> >(tee "$intake_err" >&2); then
  step_log "intake success"
  grep -E '^(interactions inserted:|interactions replayed:|skipped group chat:|evidence rows:|exact-linked handles:)' "$intake_out" >>"$LOG_FILE" || true
  inserted="$(grep -E '^interactions inserted:' "$intake_out" | tail -1 | tr -dc '0-9')"
  replayed="$(grep -E '^interactions replayed:' "$intake_out" | tail -1 | tr -dc '0-9')"
  gskip="$(grep -E '^skipped group chat:' "$intake_out" | tail -1 | tr -dc '0-9')"
  replay_check="$(grep -E '^REPLAY inserted:' "$intake_out" | tail -1)"
  step_log "PROD intake tally: inserted=${inserted:-?} replayed=${replayed:-?} skipped_group_chat=${gskip:-?} | ${replay_check:-}"
else
  step_log "intake FAILED"
  tail -60 "$intake_out" >>"$LOG_FILE" || true
  tail -60 "$intake_err" >>"$LOG_FILE" || true
  rm -f "$intake_out" "$intake_err"
  fail "PROD intake failed (export package preserved at $EXPORT_DIR); later replay completes missing work"
fi
rm -f "$intake_out" "$intake_err"

step_log "sync complete"
finish SUCCESS 0
