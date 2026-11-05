#!/usr/bin/env bash
# DEPLOY — the CulebraLuxe application to Fly.io, the production site (devops/DR-FLY.md).
#
# Same build as scripts/deploy-prod.sh, different last mile. Everything is compiled HERE, on this Mac, for free
# (devops/Dockerfile.build cross-compiles the server for x86_64 Linux and builds the Yew UI to wasm). The finished
# files are packed into the same Debian bookworm-slim runtime image (devops/Dockerfile.runtime), built on this Mac
# for linux/amd64, pushed to Fly's registry, and Fly runs that exact image.
#
#   scripts/deploy-fly.sh                        # source = Scion (ForgeVCS culebraluxe-web @ main)
#   scripts/deploy-fly.sh --source auto          # try Scion, fall back to this checkout if Scion fails
#   scripts/deploy-fly.sh --source local         # source = this checkout (use when ForgeVCS is down)
#   scripts/deploy-fly.sh --resume <tag>         # skip the build: push + deploy an image already built on this Mac
#   (default)                                    # compile AND build the image on this Mac, push it from this Mac, and
#                                                # `fly deploy --image` it. Fly builds nothing.
#   scripts/deploy-fly.sh --remote-build         # opt-in only: let a Fly remote builder build Dockerfile.runtime
#   scripts/deploy-fly.sh --no-deploy            # build and push the image only
#   scripts/deploy-fly.sh --verbose              # stream every tool's full output to the screen as well
#   scripts/deploy-fly.sh --trace                # --verbose plus set -x and every Scion request
#
# OUTPUT. The screen shows one line per step plus a heartbeat while a long step runs; every tool's full output
# (Docker, cargo, flyctl) goes to a log file under /Users/Shared/dev/build/logs/. When anything fails, the last 30
# lines of that output are printed. Every run — success, failure or Ctrl-C — ends with a REPORT: each step's
# result and time, the image, the smoke test against the Fly site, and the one command to run next.
#
# Env: FLY_APP (default culebraluxe-prod), FLY_DR_APP (default culebraluxe-dr), FORGEVCS_BASE_URL,
#      FORGEVCS_REPO, FORGEVCS_REF,
#      FORGEVCS_SECRET_FILE (default /Users/Shared/dev/.forgevcs-maestro.secret), DEPLOY_FLY_LOG_DIR
set -euo pipefail

FLY_APP="${FLY_APP:-culebraluxe-prod}"      # production (devops/fly.toml, iad)
FLY_DR_APP="${FLY_DR_APP:-culebraluxe-dr}"   # standby (devops/fly.dr.toml, ord, asleep)
DEPLOY_DR="${DEPLOY_FLY_DR:-1}"             # release to the standby too, after production passes
SOURCE="scion"
DEPLOY=1
VERBOSE=0
TRACE=0
RESUME_TAG=""
REMOTE_BUILD=0   # the Mac builds AND pushes; Fly never builds. --remote-build is opt-in only.
FORGEVCS_BASE_URL="${FORGEVCS_BASE_URL:-https://culebraluxe-forgevcs.fly.dev}"
FORGEVCS_REPO="${FORGEVCS_REPO:-culebraluxe-web}"
FORGEVCS_REF="${FORGEVCS_REF:-main}"
FORGEVCS_SECRET_FILE="${FORGEVCS_SECRET_FILE:-/Users/Shared/dev/.forgevcs-maestro.secret}"
PUSH_ATTEMPTS="${DEPLOY_FLY_PUSH_ATTEMPTS:-6}"
PUSH_ATTEMPT_TIMEOUT="${DEPLOY_FLY_PUSH_TIMEOUT:-5400}"   # per layer-set, unchanged
PUSH_BUDGET_SECS="${DEPLOY_FLY_PUSH_BUDGET:-7200}"        # total, so 6 attempts cannot mean nine hours
STAGE_FINAL="${DEPLOY_FLY_STAGE_DIR:-/Users/Shared/dev/build/fly-stage}"

# ---- run state, read by the final report --------------------------------------------------------------------------
T0=$(date +%s)
STEP_IDS=(); STEP_DESCS=(); STEP_RESULTS=(); STEP_SECS=()
CUR=-1
PUSHED=0; SOURCE_USED=""; SCION_CHANGESET=""; BUILD_SHA=""; BUILT_AT=""; TAG=""; IMAGE_REPO=""; PUSH_MODE="registry"
SMOKE_LINES=(); SMOKE_OK=""; SITE=""; FAIL_MSG=""; NEXT_HINT=""; NOTES=()
DEPLOY_DONE=0; DEPLOY_STARTED=0; SMOKE_RAN=0; PREV_IMAGE=""; ROLLBACK_CMD=""
LOG=""; WORK=""

elapsed() { local s=$(( $(date +%s) - $1 )); printf '%dm%02ds' $((s / 60)) $((s % 60)); }
say() { printf '  [%s] %s\n' "$(date +%H:%M:%S)" "$*"; [ -z "$LOG" ] || printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" >>"$LOG"; }
note() { NOTES+=("$*"); say "note: $*"; }

# ---- helpers for the deploy gate and the smoke test -----------------------------------------------------------------
# `set -x` prints the Fly and ForgeVCS tokens; --trace must not put them in the log.
trace_off() { if [ "$TRACE" = 1 ]; then set +x; fi; }
trace_on()  { if [ "$TRACE" = 1 ]; then set -x; fi; }
close_step() {  # close_step OK|FAILED|SKIPPED
  [ "$CUR" -ge 0 ] || return 0
  [ "${STEP_RESULTS[$CUR]}" = "RUNNING" ] || return 0
  STEP_RESULTS[$CUR]="$1"; STEP_SECS[$CUR]=$(( $(date +%s) - STEP_SECS[CUR] ))
}
step() {  # step "3/7" "Compiling..."
  close_step OK
  CUR=$((CUR + 1)); STEP_IDS[$CUR]="$1"; STEP_DESCS[$CUR]="$2"; STEP_RESULTS[$CUR]="RUNNING"; STEP_SECS[$CUR]=$(date +%s)
  printf '\n[%s +%s] %s %s\n' "$(date +%H:%M:%S)" "$(elapsed "$T0")" "$1" "$2"
  [ -z "$LOG" ] || printf '\n===== [%s] %s %s\n' "$(date +%H:%M:%S)" "$1" "$2" >>"$LOG"
}
skip_step() { step "$1" "$2"; close_step SKIPPED; }
fail() { FAIL_MSG="$1"; NEXT_HINT="${2:-$NEXT_HINT}"; close_step FAILED; exit 1; }
# Hard time limit for a command that could wait on a hidden prompt (macOS has no `timeout`; perl does).
with_timeout() { local secs="$1"; shift; perl -e 'alarm shift; exec @ARGV or die "exec: $!"' "$secs" "$@"; }

# quiet "label" cmd... : full output to the log (and the screen with --verbose); a heartbeat every 30 s; on failure
# the last 30 lines are shown. Never used for commands that carry secrets on argv.
quiet() {
  local label="$1"; shift
  say "$label"
  printf '\n$ %s\n' "$*" >>"$LOG"
  local rc=0 start mark; start=$(date +%s); mark=$(wc -l <"$LOG" | tr -d ' ')
  if [ "$VERBOSE" = 1 ]; then
    "$@" 2>&1 | tee -a "$LOG" || rc=${PIPESTATUS[0]}
  else
    "$@" >>"$LOG" 2>&1 </dev/null &
    local pid=$! last=$start
    while kill -0 "$pid" 2>/dev/null; do
      sleep 2
      if [ $(( $(date +%s) - last )) -ge 30 ]; then
        last=$(date +%s)
        printf '  [%s]   ...still working (%s): %s\n' "$(date +%H:%M:%S)" "$(elapsed "$start")" \
          "$(tail -n 40 "$LOG" | tr -d '\r' | grep -v '^\[' | grep -v '^\$ ' | grep -v '^ *$' | tail -n 1 | cut -c1-110)"
      fi
    done
    wait "$pid" || rc=$?
  fi
  if [ "$rc" -ne 0 ]; then
    printf '\n  ---- last 30 lines of output (full log: %s) ----\n' "$LOG"
    tail -n +"$((mark + 1))" "$LOG" | tail -n 30 | sed 's/^/  | /'
    printf '  ----\n'
  else
    say "  done in $(elapsed "$start")"
  fi
  return "$rc"
}

report() {
  local rc=$?
  set +e
  # A bare failing command (`cp`, `rsync`, a `docker` call) exits without calling fail(), so nothing named a cause.
  # Do not print "stopped unexpectedly" when the log already holds the line that explains it.
  [ -n "$FAIL_MSG" ] || [ "$rc" -eq 0 ] || FAIL_MSG="a command failed (exit $rc) without naming a cause; last log line: $(tail -n 1 "${LOG:-/dev/null}" 2>/dev/null | cut -c1-160)"
  close_step "$([ "$rc" -eq 0 ] && echo OK || echo FAILED)"
  local verdict
  if [ "$rc" -eq 130 ]; then verdict="INTERRUPTED (Ctrl-C)"
  elif [ "$SMOKE_OK" = 0 ] && [ "$SMOKE_RAN" = 1 ]; then verdict="RELEASE CREATED, APP NOT SERVING — the smoke test failed"
  elif [ "$SMOKE_OK" = 0 ]; then verdict="RELEASE CREATED, APP NOT SERVING — the new machine never answered /healthz"
  elif [ "$rc" -ne 0 ]; then verdict="FAILED"
  elif [ "$DEPLOY" = 0 ]; then verdict="BUILT AND PUSHED (not deployed: --no-deploy)"
  elif [ "$SMOKE_OK" = 1 ]; then verdict="SUCCESS — Fly is serving the new build and the smoke test passed"
  else verdict="FINISHED (no smoke test ran)"; fi
  {
    printf '\n==================================================================================================\n'
    printf ' DR DEPLOY REPORT   %s   total %s\n' "$(date '+%Y-%m-%d %H:%M')" "$(elapsed "$T0")"
    printf '==================================================================================================\n'
    printf ' RESULT:  %s\n' "$verdict"
    [ -z "$FAIL_MSG" ] || printf ' WHY:     %s\n' "$FAIL_MSG"
    printf '\n STEPS\n'
    local i
    for i in "${!STEP_IDS[@]}"; do
      local secs="${STEP_SECS[$i]}"; [ "${STEP_RESULTS[$i]}" = RUNNING ] && secs=0
      printf '   %-4s %-8s %6s  %s\n' "${STEP_IDS[$i]}" "${STEP_RESULTS[$i]}" \
        "$(printf '%dm%02ds' $((secs / 60)) $((secs % 60)))" "${STEP_DESCS[$i]}"
    done
    printf '\n BUILD\n'
    printf '   app:        %s%s\n' "$FLY_APP" "$([ "$DEPLOY_DR" = 1 ] && printf '  → standby %s' "$FLY_DR_APP")"
    printf '   source:     %s\n' "${SOURCE_USED:-not reached}"
    [ -z "$SCION_CHANGESET" ] || printf '   changeset:  %s\n' "$SCION_CHANGESET"
    printf '   git commit: %s\n' "${BUILD_SHA:-none recorded}"
    if [ -n "$TAG" ]; then
      if [ "$PUSHED" = 1 ]; then printf '   image:      %s:%s  (delivered to Fly via %s)\n' "$IMAGE_REPO" "$TAG" "$PUSH_MODE"
      else printf '   image:      %s:%s  (built on this Mac; NOT on Fly yet)\n' "$IMAGE_REPO" "$TAG"; fi
    fi
    if [ "${#SMOKE_LINES[@]}" -gt 0 ]; then
      printf '\n SMOKE TEST  %s\n' "$SITE"
      printf '   %s\n' "${SMOKE_LINES[@]}"
    fi
    if [ "${#NOTES[@]}" -gt 0 ]; then
      printf '\n NOTES\n'
      printf '   - %s\n' "${NOTES[@]}"
    fi
    if [ -n "$MACHINES" ]; then printf '\n MACHINES\n'; printf '%s\n' "$MACHINES" | sed 's/^/   /'; fi
    if [ -n "$SERVER_LOG" ]; then printf '\n SERVER LOG (fly logs, last 40 lines)\n'; printf '%s\n' "$SERVER_LOG" | cut -c1-200 | sed 's/^/   /'; fi
    [ -z "$NEXT_HINT" ] || printf '\n NEXT\n   %s\n' "$NEXT_HINT"
    [ -z "$LOG" ] || printf '\n Full log: %s\n' "$LOG"
    printf '==================================================================================================\n'
  } | tee -a "${LOG:-/dev/null}"
  [ -z "$WORK" ] || rm -rf "$WORK"
}
# What the machine itself printed — the reason a deploy "times out waiting for health checks" is almost always here.
SERVER_LOG=""; MACHINES=""
fly_diagnose() {
  MACHINES="$(with_timeout 45 "$FLY" machine list --app "$FLY_APP" </dev/null 2>&1 | grep -E '^[0-9a-f]{14}|STATE|ID' | head -6 || true)"
  SERVER_LOG="$(with_timeout 45 "$FLY" logs --app "$FLY_APP" --no-tail </dev/null 2>&1 | tail -n 40 || true)"
  printf '\n--- fly machine list ---\n%s\n--- fly logs (last 40) ---\n%s\n' "$MACHINES" "$SERVER_LOG" >>"$LOG"
}
on_interrupt() {
  # After `fly deploy` returns, a release HAS been created; claiming "nothing new was deployed" would be false.
  if [ "$DEPLOY_DONE" = 1 ]; then
    FAIL_MSG="interrupted after the release was created (step ${STEP_IDS[$CUR]:-?}); check whether the new machine is serving before trusting either version"
  elif [ "$DEPLOY_STARTED" = 1 ]; then
    FAIL_MSG="interrupted DURING the deploy (step ${STEP_IDS[$CUR]:-?}) — a release may already exist; the previous machines should still be serving unless the deploy completed"
  else
    FAIL_MSG="interrupted during step ${STEP_IDS[$CUR]:-?}; nothing new was deployed by this run"
  fi
  exit 130
}
trap report EXIT
trap on_interrupt INT TERM

while [ $# -gt 0 ]; do
  case "$1" in
    --source) SOURCE="${2:-}"; shift 2 ;;
    --source=*) SOURCE="${1#*=}"; shift ;;
    --no-deploy) DEPLOY=0; shift ;;
    --no-dr) DEPLOY_DR=0; shift ;;
    --resume) RESUME_TAG="${2:-}"; shift 2 ;;
    --remote-build) REMOTE_BUILD=1; shift ;;
    --push) REMOTE_BUILD=0; shift ;;
    --app) FLY_APP="${2:-}"; shift 2 ;;
    --ref) FORGEVCS_REF="${2:-}"; shift 2 ;;
    --verbose|-v) VERBOSE=1; shift ;;
    --trace) TRACE=1; VERBOSE=1; shift ;;
    -h|--help) trap - EXIT; sed -n '2,27p' "$0"; exit 0 ;;
    *) fail "Unknown option: $1 (see --help)" ;;
  esac
done
case "$SOURCE" in scion|local|auto) ;; *) fail "--source must be scion, local or auto" ;; esac
[ "$TRACE" = 0 ] || set -x

FLY="$(command -v fly || command -v flyctl || true)"
[ -n "$FLY" ] || fail "flyctl is not installed" "brew install flyctl && fly auth login"
ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null)" || fail "not inside a CulebraLuxe checkout" "cd /Users/Shared/dev/src/lane-claude"
cd "$ROOT_DIR"
LOG_DIR="${DEPLOY_FLY_LOG_DIR:-/Users/Shared/dev/build/logs}"
mkdir -p "$LOG_DIR" 2>/dev/null || LOG_DIR="${TMPDIR:-/tmp}"
LOG="$LOG_DIR/deploy-fly-$(date +%Y%m%d-%H%M%S).log"
( umask 077; : >"$LOG" )
"$FLY" auth whoami >/dev/null 2>&1 </dev/null || fail "flyctl is not logged in" "fly auth login"
# Docker and tailwindcss are needed only to BUILD. `--resume` exists to push/deploy what is already on this Mac
# (DR-FLY.md calls it the recovery path), so a closed Docker Desktop or an absent node_modules must not stop it
# before it starts; --remote-build compiles on Fly's builder and needs neither.
if [ "$REMOTE_BUILD" = 0 ]; then
  docker info >/dev/null 2>&1 || fail "Docker is not running" "start Docker Desktop, wait for 'running', then re-run"
fi
TAILWIND=""
if [ -z "$RESUME_TAG" ]; then
  TAILWIND="$ROOT_DIR/node_modules/.bin/tailwindcss"
  [ -x "$TAILWIND" ] || fail "no tailwindcss in $ROOT_DIR/node_modules" "pnpm install"
fi
IMAGE_REPO="registry.fly.io/${FLY_APP}"
WORK="$(mktemp -d)"; chmod 700 "$WORK"

printf '\nCulebraLuxe DR deploy -> Fly.io app %s  (source: %s%s)\n' "$FLY_APP" "$SOURCE" "${RESUME_TAG:+, resuming $RESUME_TAG}"
printf '  checkout: %s\n  log:      %s\n' "$ROOT_DIR" "$LOG"
{ printf 'flyctl: %s\n' "$("$FLY" version 2>/dev/null | head -1)"
  printf 'docker: %s\n' "$(docker version --format '{{.Server.Version}} {{.Server.Arch}}' 2>/dev/null)"; } >>"$LOG"

# The app has to exist before anything reads its secrets; create it here so a first run can get this far.
with_timeout 60 "$FLY" status --app "$FLY_APP" </dev/null >/dev/null 2>&1 \
  || quiet "creating the Fly app $FLY_APP" with_timeout 120 "$FLY" apps create "$FLY_APP" \
  || fail "could not create the Fly app $FLY_APP"

if [ -n "$RESUME_TAG" ]; then
  # ---- resume: the image is already built and tagged on this Mac -------------------------------------------------
  TAG="${RESUME_TAG##*:}"
  if [ "$REMOTE_BUILD" = 1 ]; then
    # Resume = deploy the files the last build left in the stable stage dir, through the remote builder.
    [ -f "$STAGE_FINAL/Dockerfile" ] || fail "no previous stage at $STAGE_FINAL to resume" "scripts/deploy-fly.sh --source auto"
    STAGE="$STAGE_FINAL"
    SOURCE_USED="resumed stage $STAGE_FINAL"
    BUILD_SHA="$(sed -n 's/^ENV CULEBRALUXE_BUILD_SHA=//p' "$STAGE/Dockerfile")"
    SCION_CHANGESET="$(sed -n 's/^ENV CULEBRALUXE_SCION_CHANGESET=//p' "$STAGE/Dockerfile")"
    BUILT_AT="$(sed -n 's/^ENV CULEBRALUXE_BUILT_AT=//p' "$STAGE/Dockerfile")"
  else
    docker image inspect "$IMAGE_REPO:$TAG" >/dev/null 2>&1 \
      || fail "no local image $IMAGE_REPO:$TAG to resume" "docker images $IMAGE_REPO   (or run without --resume to rebuild)"
    # Fly machines are x86_64; the build path passes --platform linux/amd64 for exactly that reason. A resumed tag
    # was never checked, so an arm64 image built by mistake would push fine and die on the machine.
    RESUME_PLATFORM="$(docker image inspect "$IMAGE_REPO:$TAG" --format '{{.Os}}/{{.Architecture}}' 2>/dev/null || true)"
    [ "$RESUME_PLATFORM" = "linux/amd64" ] || fail "local image $IMAGE_REPO:$TAG is '${RESUME_PLATFORM:-unknown}', not linux/amd64 — Fly machines are x86_64" "scripts/deploy-fly.sh   (rebuild for linux/amd64)"
    SOURCE_USED="resumed local image (stamps are self-declared by the image)"
    IMG_ENV="$(docker image inspect "$IMAGE_REPO:$TAG" --format '{{range .Config.Env}}{{println .}}{{end}}')"
    BUILD_SHA="$(printf '%s\n' "$IMG_ENV" | sed -n 's/^CULEBRALUXE_BUILD_SHA=//p')"
    SCION_CHANGESET="$(printf '%s\n' "$IMG_ENV" | sed -n 's/^CULEBRALUXE_SCION_CHANGESET=//p')"
    BUILT_AT="$(printf '%s\n' "$IMG_ENV" | sed -n 's/^CULEBRALUXE_BUILT_AT=//p')"
  fi
  for s in "1/7|Getting the source" "2/7|Building the stylesheet" "3/7|Compiling the application" "4/7|Packing the finished files"; do
    skip_step "${s%%|*}" "${s#*|} (skipped: --resume)"
  done
  if [ "$REMOTE_BUILD" = 1 ]; then step 5/7 "Image: the Fly remote builder rebuilds it from $STAGE_FINAL"; else step 5/7 "Pushing the image to Fly..."; fi
else
# ---------------------------------------------------------------------------------------------------------------
step 1/7 "Getting the source..."
# Returns non-zero (never exits) so --source auto can fall back to the local checkout.
get_scion_source() {
  [ -f "$FORGEVCS_SECRET_FILE" ] || { say "no ForgeVCS secret at $FORGEVCS_SECRET_FILE"; return 1; }
    local health
  health="$(curl -s -o /dev/null -w '%{http_code} %{time_total}s' --max-time 15 \
    "$FORGEVCS_BASE_URL/health" </dev/null || true)"
  say "ForgeVCS health: ${health:-no answer}"
  case "$health" in 200*) ;; *) say "ForgeVCS at $FORGEVCS_BASE_URL is not answering (it is hosted on Fly)"; return 1 ;; esac
  TOKEN="$(python3 -c 'import json,sys; print(json.dumps({"service":"maestro","secret":open(sys.argv[1]).read().strip()}))' \
      "$FORGEVCS_SECRET_FILE" |
    curl -sS --max-time 30 -H 'content-type: application/json' \
      -X POST --data-binary @- "$FORGEVCS_BASE_URL/auth/token" 2>>"$LOG" |
    python3 -c 'import json,sys; print(json.load(sys.stdin).get("token",""))' 2>/dev/null || true)"
  [ -n "$TOKEN" ] || { say "could not mint a ForgeVCS token"; return 1; }
  SRC="$WORK/src"
  local flags=()
  [ "$TRACE" = 0 ] || flags=(-v)
  say "exporting $FORGEVCS_REPO@$FORGEVCS_REF from Scion (progress below; 15 min limit)..."
  # The export's progress lines are short and useful, so they stay on screen (and go to the log).
  trace_off   # the assignment below would be echoed by set -x with the token in it
  EXPORT_JSON="$( { FORGEVCS_TOKEN="$TOKEN" python3 "$ROOT_DIR/scripts/forge-export.py" \
      --base "$FORGEVCS_BASE_URL" --repo "$FORGEVCS_REPO" --ref "$FORGEVCS_REF" --dest "$SRC" ${flags[@]+"${flags[@]}"} \
      </dev/null 2> >(tee -a "$LOG" | grep --line-buffered -v 'listing:' >&2); } )" \
    || { trace_on; say "the Scion export failed (details above and in the log)"; return 1; }
  trace_on
  SCION_CHANGESET="$(printf '%s' "$EXPORT_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin)["changeset"])' || true)"
  BUILD_SHA="$(printf '%s' "$EXPORT_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin)["git_commit"])' || true)"
  # This function is called in a `||` (and an `if !`) context, where bash disables errexit for its whole body, so
  # every command that can fail has to say so itself — otherwise a failed parse or copy returns 0 and the build
  # carries an empty changeset (or missing public/) and nobody knows until the site is wrong.
  [ -n "$SCION_CHANGESET" ] || { say "the Scion export returned no changeset id"; return 1; }
  say "Scion export: $(printf '%s' "$EXPORT_JSON" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("%d files, %.1f MB, %ss" % (d["files"], d["bytes"]/1e6, d["seconds"]))')"
  [ -n "$BUILD_SHA" ] || note "the Scion changeset names no git commit; /api/build-info reports the changeset as its build identity"
  # The Scion mirror is SOURCE-ONLY: it excludes public/ (66 MB of images), so the static files come from this checkout.
  mkdir -p "$SRC/public" || { say "could not create $SRC/public"; return 1; }
  rsync -a --exclude rust-ui --exclude 'upload/data' "$ROOT_DIR/public/" "$SRC/public/" \
    || { say "could not copy public/ from $ROOT_DIR"; return 1; }
  # The code comes from Scion main but public/ (images, the built CSS) comes from THIS checkout. When the two trees
  # differ the image ships static files older than its code, which looks like a rendering bug and is not one.
  LOCAL_BEHIND="$(git -C "$ROOT_DIR" rev-list --count HEAD..origin/main 2>/dev/null || echo 0)"
  [ "${LOCAL_BEHIND:-0}" -eq 0 ] || note "public/ in this build came from $ROOT_DIR, which is $LOCAL_BEHIND commit(s) behind main — the code came from Scion"
  # Tailwind resolves `@import 'tw-animate-css'` / `shadcn/tailwind.css` from node_modules next to the source.
  ln -s "$ROOT_DIR/node_modules" "$SRC/node_modules" || { say "could not link node_modules into $SRC"; return 1; }
  SOURCE_USED="Scion $FORGEVCS_REPO@$FORGEVCS_REF (+ public/ from $ROOT_DIR)"
  TAG="scion-${SCION_CHANGESET:0:20}-$(date -u +%Y%m%d%H%M%S)"
}
use_local_source() {
  SRC="$ROOT_DIR"; SCION_CHANGESET=""
  BUILD_SHA="$(git rev-parse HEAD)"
  # THIS WORKTREE IS OFTEN NOT ON main. On 2026-11-03 lane-claude was a DETACHED HEAD 214 commits behind main, and
  # `--source local` is the documented path for when ForgeVCS is down — so the one run that matters most would
  # have deployed three weeks of stale code and said nothing. Say it in the report.
  if [ "$(git rev-parse --abbrev-ref HEAD 2>/dev/null)" = "HEAD" ]; then
    note "the local checkout is a DETACHED HEAD (${BUILD_SHA:0:12}) — this build is that commit, not main"
  fi
  LOCAL_BEHIND="$(git rev-list --count HEAD..origin/main 2>/dev/null || echo 0)"
  [ "${LOCAL_BEHIND:-0}" -eq 0 ] || note "the local checkout is $LOCAL_BEHIND commit(s) behind origin/main — a --source local build is stale by that much"
  SOURCE_USED="local checkout $ROOT_DIR @ ${BUILD_SHA:0:12}"
  git diff --quiet HEAD -- 2>/dev/null || note "the local checkout has uncommitted changes; they are in this build"
  say "source: $SOURCE_USED"
  TAG="git-${BUILD_SHA:0:12}-$(date -u +%Y%m%d%H%M%S)"
}
case "$SOURCE" in
  scion) get_scion_source || fail "could not get the source from Scion" "scripts/deploy-fly.sh --source auto" ;;
  auto)
    if ! get_scion_source; then
      note "Scion failed, so this build FELL BACK to the local checkout (--source auto)"
      rm -rf "$WORK/src" 2>/dev/null || true
      use_local_source
    fi ;;
  local) use_local_source ;;
esac
TAG="$(printf '%s' "$TAG" | tr -c 'A-Za-z0-9_.-' '-')"
for need in devops/Dockerfile.build devops/Dockerfile.runtime web/ui/styles/app.css scripts/rust-ui-build.sh; do
  [ -e "$SRC/$need" ] || fail "the source has no $need"
done
# Every `COPY <src> <dst>` in the build file must exist BEFORE Docker starts (2026-11-03: "/observability": not found).
MISSING="$(awk '/^COPY / && $2 !~ /^--/ { for (i = 2; i < NF; i++) print $i }' "$SRC/devops/Dockerfile.build" |
  while read -r p; do [ -e "$SRC/$p" ] || printf '%s ' "$p"; done)"
[ -z "$MISSING" ] || fail "devops/Dockerfile.build copies paths the source does not have: $MISSING"
say "build-file check: every COPY source is present"

# ---------------------------------------------------------------------------------------------------------------
step 2/7 "Building the stylesheet..."
quiet "tailwindcss -> public/app.css" bash -c 'cd "$1" && "$2" -i web/ui/styles/app.css -o public/app.css --minify' _ "$SRC" "$TAILWIND" \
  || fail "the stylesheet build failed"

step 3/7 "Compiling the application on this Mac (first run ~10 min, cached after)..."
quiet "docker build (Rust server + wasm UI, cross-compiled for x86_64)" \
  docker build --progress=plain -f "$SRC/devops/Dockerfile.build" --output "type=local,dest=$WORK/build" "$SRC" \
  || fail "the local compile failed (the cargo/docker error is shown above)"
[ -f "$WORK/build/culebraluxe" ] && [ -f "$WORK/build/ui_bg.wasm" ] || fail "the compile produced no server or no wasm"
say "server binary $(du -h "$WORK/build/culebraluxe" | cut -f1), wasm $(du -h "$WORK/build/ui_bg.wasm" | cut -f1)"

# ---------------------------------------------------------------------------------------------------------------
step 4/7 "Packing the finished files..."
# Identical to deploy-prod.sh step 3 (same exclusions, same mode normalization, same fail-closed private-data probe).
# Built in a scratch dir, then synced into a STABLE stage dir. The stable path is what makes uploads small: BuildKit
# keys its context cache by the context directory, and with the same path and unchanged mtimes it only sends files that
# changed (in practice the server binary and wasm, ~15 MB, instead of all ~80 MB). A random temp dir defeated that.
STAGE="$WORK/stage"
mkdir -p "$STAGE/public/rust-ui" "$STAGE/templates"
cp "$SRC/devops/Dockerfile.runtime" "$STAGE/Dockerfile"
BUILT_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"   # /api/build-info reports this, so the smoke test can prove WHICH image serves
printf 'ENV CULEBRALUXE_BUILD_SHA=%s\nENV CULEBRALUXE_BUILT_AT=%s\nENV CULEBRALUXE_SCION_CHANGESET=%s\nENV CULEBRALUXE_DEPLOY_TARGET=fly\n' \
  "$BUILD_SHA" "$BUILT_AT" "$SCION_CHANGESET" >> "$STAGE/Dockerfile"
gzip -9 -n -c "$WORK/build/culebraluxe" > "$STAGE/culebraluxe.gz"   # -n: no timestamp, same input -> same bytes
rsync -a --exclude rust-ui --exclude 'upload/data' --exclude '* 2.*' --exclude '* 2' "$SRC/public/" "$STAGE/public/"
cp "$WORK/build/ui.js" "$STAGE/public/rust-ui/ui.js"
gzip -9 -n -c "$WORK/build/ui_bg.wasm" > "$STAGE/public/rust-ui/ui_bg.wasm.gz"
rsync -a "$SRC/middle/model/forms/templates/" "$STAGE/templates/"
chmod -R a+rX "$STAGE/public" "$STAGE/templates"
# Content-addressed asset names (scripts/asset-hash.sh): the shell names these, never the stable names.
quiet "content-addressed asset names" bash "$ROOT_DIR/scripts/asset-hash.sh" "$STAGE/public" \
  || fail "the asset hashing step failed" 
PRIVATE_MATCH="$(find "$STAGE" \( -path '*/upload/data/*' -o -path '*/apple-messages-export/*' \
  -o -path '*/contact-export/*' -o -path '*/apple-messages-output/*' -o -name 'culebraluxe-calendar*.json' \) \
  -print -quit 2>/dev/null || true)"
[ -z "$PRIVATE_MATCH" ] || fail "private local data reached the deploy stage: $PRIVATE_MATCH — not deploying this artifact"
mkdir -p "$STAGE_FINAL" && chmod 700 "$STAGE_FINAL"
# --checksum: a regenerated but identical file keeps the old copy (and its mtime), so it is not re-uploaded.
CHANGED="$(rsync -a --delete --checksum --out-format='%n' "$STAGE/" "$STAGE_FINAL/" | grep -v '/$' | wc -l | tr -d ' ')"
STAGE="$STAGE_FINAL"
say "private-data probe: clean; stage $(du -sh "$STAGE" | cut -f1) at $STAGE ($CHANGED files changed since the last run)"

# ---------------------------------------------------------------------------------------------------------------
if [ "$REMOTE_BUILD" = 1 ]; then
  step 5/7 "Image: built by a Fly remote builder from the packed files (--remote-build; no docker push)"
  [ "$DEPLOY" = 1 ] || fail "--remote-build needs the deploy step (do not combine with --no-deploy)"
else
step 5/7 "Building the runtime image and pushing it to Fly..."
# Fly machines are x86_64; the Mac is ARM, so the platform is explicit (the runtime stage only unpacks files).
quiet "docker buildx (linux/amd64 runtime image $TAG)" \
  docker buildx build --progress=plain --platform linux/amd64 --load -t "$IMAGE_REPO:$TAG" "$STAGE" \
  || fail "the runtime image build failed"
fi
fi

# (the app is created before the configuration preflight, so it already exists here)
# The registry login lasts only a few minutes and a slow uplink can outlive it, so every attempt logs in afresh.
# 2026-11-03: the first push died with "timeout awaiting response headers" on one layer from the island connection.
# PUSHING FROM THE MAC OVER A SLOW, VARIABLE LINK (Starlink, ~2 Mbit/s up measured 2026-11-03).
#
# `docker push` failed every time on the same layer with "timeout awaiting response headers": Docker's pusher gives the
# registry a fixed ~30 s to answer after a layer upload, and `fly auth docker` credentials expire after ~3 minutes. Both
# limits are built into the tools, not set by this script, so the push uses `crane` (go-containerregistry) when it is
# installed: no response-header deadline, it skips layers the registry already has, and it logs in with the long-lived
# `fly auth token` instead of the 3-minute registry credential. Every attempt resumes: layers that landed stay landed.
# Docker push remains the fallback when crane is not installed.
CRANE="$(command -v crane || true)"
# `fly auth docker` mints a registry credential that expires in ~3 minutes — useless on this uplink — so crane is
# logged in with the long-lived `fly auth token`. Re-mint THAT before every attempt: a retry that ran
# `fly auth docker` overwrote the very credential crane needed, and the next attempt went out with a dead one.
crane_login() {
  local tok rc=0
  trace_off   # the token must reach neither the screen nor the log: not while fetched, not while piped to crane
  tok="$(with_timeout 30 "$FLY" auth token </dev/null 2>/dev/null | tail -n 1 || true)"
  if [ -z "$tok" ]; then trace_on; return 1; fi
  printf '%s' "$tok" | "$CRANE" auth login registry.fly.io -u x --password-stdin >>"$LOG" 2>&1 || rc=$?
  trace_on
  return "$rc"
}
pushed=0
if [ "$REMOTE_BUILD" = 1 ]; then pushed=1; PUSHED=1; PUSH_MODE="Fly remote builder (--remote-build)"; fi
if [ "$pushed" = 0 ] && [ -n "$CRANE" ]; then
  PUSH_MODE="crane push from this Mac"
  crane_login || note "crane could not store the registry login; it will use Docker's credentials instead"
  quiet "exporting the image for upload (docker save)" docker save -o "$WORK/image.tar" "$IMAGE_REPO:$TAG" \
    || fail "docker save failed"
fi
# A total wall-clock budget, not only a per-attempt timeout: 6 attempts x 5400 s used to allow nine hours of retrying
# a dead connection. Layers that landed stay landed, so a later run resumes instead of restarting.
PUSH_DEADLINE=$(( $(date +%s) + PUSH_BUDGET_SECS ))
[ "$pushed" = 1 ] || for attempt in $(seq 1 "$PUSH_ATTEMPTS"); do
  if [ "$(date +%s)" -ge "$PUSH_DEADLINE" ]; then
    say "push budget of ${PUSH_BUDGET_SECS}s exhausted; not starting attempt $attempt"
    break
  fi
  if [ -n "$CRANE" ]; then
    [ "$attempt" = 1 ] || crane_login || true
    quiet "crane push, attempt $attempt/$PUSH_ATTEMPTS (only layers Fly does not already have; no time limit per layer)" \
      with_timeout "$PUSH_ATTEMPT_TIMEOUT" "$CRANE" push "$WORK/image.tar" "$IMAGE_REPO:$TAG" && { pushed=1; PUSHED=1; break; }
  else
    PUSH_MODE="docker push from this Mac"
    with_timeout 60 "$FLY" auth docker </dev/null >>"$LOG" 2>&1 || { say "fly auth docker failed (attempt $attempt)"; sleep 10; continue; }
    quiet "docker push, attempt $attempt/$PUSH_ATTEMPTS" docker push "$IMAGE_REPO:$TAG" && { pushed=1; PUSHED=1; break; }
  fi
  if [ "$attempt" -lt "$PUSH_ATTEMPTS" ]; then
    say "push attempt $attempt did not finish; layers already uploaded are kept. Retrying in $((attempt * 20)) s..."
    sleep $((attempt * 20))
  fi
done
if [ "$pushed" = 0 ]; then
  if [ -z "$CRANE" ]; then
    fail "docker push could not finish over this connection after $PUSH_ATTEMPTS attempts" \
      "brew install crane   then: scripts/deploy-fly.sh --resume $TAG   (pushes the image already built here; no rebuild)"
  fi
  fail "the push did not finish after $PUSH_ATTEMPTS attempts (see the last output above)" \
    "scripts/deploy-fly.sh --resume $TAG   (resumes the upload; layers already on Fly are skipped)"
fi
[ -n "$CRANE" ] || note "install crane (brew install crane) for pushes that tolerate slow uploads; this run used docker push"

# ---------------------------------------------------------------------------------------------------------------

# ---------------------------------------------------------------------------------------------------------------
# What is serving RIGHT NOW, so a bad release can name the image to go back to. The old failure path told the
# operator to roll back to "$IMAGE_REPO:<previous tag>" — a literal placeholder, i.e. not a command. Only a release
# whose STATUS is `complete` may be offered: after a failed deploy the newest row IS the broken image.
PREV_IMAGE="$(with_timeout 45 "$FLY" releases --app "$FLY_APP" --image </dev/null 2>/dev/null | awk '$3 == "complete" && $NF ~ /^registry\./ {print $NF; exit}' || true)"

if [ "$DEPLOY" = 0 ]; then
  skip_step 6/7 "Deploy (skipped: --no-deploy)"
  NEXT_HINT="scripts/deploy-fly.sh --resume $TAG"
  exit 0
fi
step 6/7 "Deploying to Fly and running the smoke test..."
# BLUEGREEN, not rolling: there is exactly ONE standby machine, so a rolling update takes the only copy down for the
# whole pull and boot — and leaves it down if the image crash-loops. Bluegreen starts the new machine, waits for its
# /healthz, and only then removes the old one, so a bad release keeps the previous version serving.
DEPLOY_STRATEGY="${DEPLOY_FLY_STRATEGY:-bluegreen}"
DEPLOY_STARTED=1   # from here a release may exist even if this shell is interrupted
if [ "$REMOTE_BUILD" = 0 ]; then
  quiet "fly deploy --image $IMAGE_REPO:$TAG (--strategy $DEPLOY_STRATEGY)" with_timeout 900 "$FLY" deploy --app "$FLY_APP" \
    --config "$ROOT_DIR/devops/fly.toml" --image "$IMAGE_REPO:$TAG" --strategy "$DEPLOY_STRATEGY" --wait-timeout 300 \
    || { fly_diagnose; fail "fly deploy failed — see the deploy output above (the previous release should still be serving)" "if it reports multiple image versions, remove the leftover machine first: fly machine list --app $FLY_APP   — otherwise read SERVER LOG above, fix, then: scripts/deploy-fly.sh --resume $TAG"; }
else
  quiet "fly deploy via remote builder (uploads changed files only; no time limit tighter than 90 min)" with_timeout 5400 "$FLY" deploy "$STAGE" --app "$FLY_APP" \
    --config "$ROOT_DIR/devops/fly.toml" --dockerfile "$STAGE/Dockerfile" --remote-only --image-label "$TAG" \
    --strategy "$DEPLOY_STRATEGY" --wait-timeout 300 \
    || { fly_diagnose; fail "fly deploy (remote builder) failed: the new machine never became healthy" "read SERVER LOG above, fix, then: scripts/deploy-fly.sh --source auto --remote-build   (the compile is cached)"; }
fi
DEPLOY_DONE=1   # a release now exists, whatever happens next

SITE="https://${FLY_APP}.fly.dev"
# `fly deploy` returning 0 is NOT evidence the new release serves: on 2026-11-03 it printed
# "Machine … reached stopped state / ✔ … is now in a good state" after 14 s for a machine that was crash-looping, and
# the smoke test then burned three minutes against a dead app. /healthz is the evidence; not getting it is a FAILED
# deploy, not a smoke test with one bad row.
say "waiting for $SITE/healthz to answer..."
HEALTHZ_OK=0; HEALTHZ_LAST="000"
for i in $(seq 1 18); do
  HEALTHZ_LAST="$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "$SITE/healthz" </dev/null || true)"
  [ -n "$HEALTHZ_LAST" ] || HEALTHZ_LAST="000"
  if [ "$HEALTHZ_LAST" = 200 ]; then HEALTHZ_OK=1; say "/healthz answered 200 (attempt $i/18)"; break; fi
  say "  /healthz -> ${HEALTHZ_LAST:-000} (attempt $i/18); retrying in 5s"
  sleep 5
done
if [ "$HEALTHZ_OK" = 0 ]; then
  SMOKE_OK=0
  fly_diagnose   # this is the thing that was missing on 2026-11-03: the machine list and the server's own log
  ROLLBACK_CMD=""
  [ -z "$PREV_IMAGE" ] || ROLLBACK_CMD="fly deploy --app $FLY_APP --config devops/fly.toml --image $PREV_IMAGE"
  fail "the new release never answered /healthz (last: ${HEALTHZ_LAST:-000}) — the machine is not serving; the SERVER LOG below names why" \
       "${ROLLBACK_CMD:-fly releases --app $FLY_APP --image   (no previous release was recorded)}"
fi

SMOKE_RAN=1   # a /healthz answer means the checks below really run; the verdict may now blame them
SMOKE_OK=1; SMOKE_BAD=0
smoke_one() {  # smoke_one PATH — one check; can never end the run
  local path="$1" out code secs mark line
  out="$(curl -s -o /dev/null -w '%{http_code} %{time_total}' --max-time 30 "${SITE}${path}" </dev/null || true)"
  # curl writes its -w string even when it FAILS ("000 30.007…" on a timeout). The old code added `|| echo "000 0"`,
  # which APPENDED to that string instead of replacing it: "000 30.007773000 0" -> `printf %5.2f` -> "invalid number"
  # -> `set -e` killed the run inside check 2 of 9. Read the first two fields of the first line, only.
  code="000"; secs="0"
  if [ -n "$out" ]; then
    read -r code secs _ <<<"$out" || true
    [ -n "$code" ] || code="000"
    case "$secs" in ''|*[!0-9.]*) secs="0" ;; esac
  fi
  mark=PASS
  if [ "$code" = 200 ]; then :; else mark=FAIL; SMOKE_OK=0; SMOKE_BAD=$((SMOKE_BAD + 1)); fi
  # Cosmetic formatting must not be able to abort a deploy: fall back to a plain line rather than let printf fail.
  if ! printf -v line '%s  %s  %5.2fs  %s' "$mark" "$code" "$secs" "$path" 2>/dev/null; then
    line="$mark  $code  ?s  $path"
  fi
  SMOKE_LINES+=("$line")
}
for path in /healthz / /buyers /app.css /rust-ui/ui.js /rust-ui/ui_bg.wasm "/api/rust-ui/public-page?screen=site-home" /login /api/build-info; do
  smoke_one "$path"
done

# NINE 200s PROVE *A* BUILD IS SERVING, NOT THIS ONE. A machine that kept the previous release passes every URL — the
# old script called that "SUCCESS — Fly is serving the new build". /api/build-info reports CULEBRALUXE_BUILT_AT, so
# compare it with the stamp this image carries.
BUILD_INFO_JSON="$(curl -s --max-time 15 "$SITE/api/build-info" </dev/null || true)"
BUILD_INFO="$(printf '%s' "$BUILD_INFO_JSON" | head -c 300)"
[ -z "$BUILD_INFO" ] || SMOKE_LINES+=("build-info: $BUILD_INFO")
if [ -n "$BUILT_AT" ]; then
  SERVED_AT="$(printf '%s' "$BUILD_INFO_JSON" | python3 -c 'import json,sys
try: print(json.load(sys.stdin).get("builtAt") or "")
except Exception: print("")' 2>/dev/null || true)"
  if [ "$SERVED_AT" != "$BUILT_AT" ]; then
    SMOKE_OK=0; SMOKE_BAD=$((SMOKE_BAD + 1))
    SMOKE_LINES+=("FAIL  the machine is not serving this image (this image builtAt=$BUILT_AT, /api/build-info says builtAt=${SERVED_AT:-<nothing>})")
  fi
else
  note "the image carries no CULEBRALUXE_BUILT_AT stamp; the smoke test cannot prove the new image is the one serving"
fi
# A Scion-sourced build has no git sha: the changeset IS its identity, so the machine must report the changeset this
# build exported. Without this the builtAt check alone cannot tell two Scion builds apart.
if [ -n "$SCION_CHANGESET" ]; then
  SERVED_CHANGESET="$(printf '%s' "$BUILD_INFO_JSON" | python3 -c 'import json,sys
try: print(json.load(sys.stdin).get("changeset") or "")
except Exception: print("")' 2>/dev/null || true)"
  if [ "$SERVED_CHANGESET" != "$SCION_CHANGESET" ]; then
    SMOKE_OK=0; SMOKE_BAD=$((SMOKE_BAD + 1))
    SMOKE_LINES+=("FAIL  the machine is not serving this image (this build exported changeset $SCION_CHANGESET, /api/build-info says ${SERVED_CHANGESET:-<nothing>})")
  fi
fi
# The other half of the same question: a machine that boots is not a success if it booted against the DEV database.
# /api/build-info reports the target for exactly this reason (web/src/api/build_info.rs: "a live container pointed at
# DEV is a failed release, not a partial success").
SERVED_DB="$(printf '%s' "$BUILD_INFO_JSON" | python3 -c 'import json,sys
try: print(json.load(sys.stdin).get("databaseTarget") or "")
except Exception: print("")' 2>/dev/null || true)"
if [ -n "$SERVED_DB" ] && [ "$SERVED_DB" != "prod" ]; then
  SMOKE_OK=0; SMOKE_BAD=$((SMOKE_BAD + 1))
  SMOKE_LINES+=("FAIL  the app resolved the database target '$SERVED_DB', not 'prod' — a site pointed at DEV is a failed release")
fi

# INTEGRATIONS — photos, Mux, WhatsApp, LuxeSign, email and the settings behind them (scripts/smoke-site.sh,
# read-only). They are reported, not gating: the 9 checks above decide whether this release serves; a FAIL here
# names a feature that needs attention on a site that is up.
if [ -x "$ROOT_DIR/scripts/smoke-site.sh" ]; then
  INTEG="$("$ROOT_DIR/scripts/smoke-site.sh" --app "$FLY_APP" --base "$SITE" 2>>"$LOG" </dev/null \
    | grep -E '^(PASS|FAIL|WARN|MANUAL) ' | grep -v -E '^[A-Z]+ +core ' || true)"
  printf '\n--- smoke-site.sh ---\n%s\n' "$INTEG" >>"$LOG"
  if [ -n "$INTEG" ]; then
    SMOKE_LINES+=("--- integrations (scripts/smoke-site.sh) ---")
    while IFS= read -r l; do [ -z "$l" ] || SMOKE_LINES+=("$l"); done <<<"$INTEG"
    INTEG_FAILS="$(printf '%s\n' "$INTEG" | grep -c '^FAIL' || true)"
    [ "${INTEG_FAILS:-0}" = 0 ] || note "$INTEG_FAILS integration check(s) FAILED (listed under SMOKE TEST); the site itself is serving"
  fi
fi

# The standby takes the SAME image as the release that just passed, and only after it passed: a DR app carrying a
# build that failed its smoke is worse than a DR app one release behind.
if [ "$SMOKE_OK" = 1 ] && [ "$DEPLOY_DR" = 1 ]; then
  step 7/7 "Updating the standby $FLY_DR_APP to this image (it stays asleep)..."
  quiet "fly deploy --app $FLY_DR_APP (devops/fly.dr.toml, $FLY_DR_APP.fly.dev)" with_timeout 600 "$FLY" deploy \
    --app "$FLY_DR_APP" --config "$ROOT_DIR/devops/fly.dr.toml" --image "$IMAGE_REPO:$TAG" \
    --strategy bluegreen --wait-timeout 300 \
    || fail "the standby did not take the image" "fly logs --app $FLY_DR_APP   ·   fly deploy --app $FLY_DR_APP --image $IMAGE_REPO:$TAG"
  DR_SITE="https://${FLY_DR_APP}.fly.dev"
  DR_CODE="$(curl -s -o /dev/null -w '%{http_code}' --max-time 60 "$DR_SITE/healthz" </dev/null || true)"
  [ "$DR_CODE" = 200 ] || fail "the standby answers /healthz with ${DR_CODE:-nothing} (a suspended machine may need a moment to wake)" \
    "fly logs --app $FLY_DR_APP"
  SMOKE_LINES+=("PASS  standby $DR_SITE/healthz -> 200")
  close_step OK
fi

if [ "$SMOKE_OK" = 1 ]; then
  NEXT_HINT="open $SITE"
else
  FAIL_MSG="$SMOKE_BAD smoke check(s) failed"
  ROLLBACK_CMD=""
  [ -z "$PREV_IMAGE" ] || ROLLBACK_CMD="fly deploy --app $FLY_APP --config devops/fly.toml --image $PREV_IMAGE"
  NEXT_HINT="fly logs --app $FLY_APP   ·   restore the previous release: ${ROLLBACK_CMD:-fly releases --app $FLY_APP --image   (none recorded)}"
  exit 1
fi
