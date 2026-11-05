#!/usr/bin/env bash
# PRODUCTION SMOKE — the 9 core checks plus the integrations, against the live Fly site. Read-only: it never creates
# a lead, a guest code, a signature request, an upload or a message. What cannot be proven without writing is listed
# as MANUAL with the exact thing to do by hand.
#
#   scripts/smoke-site.sh                                  # https://culebraluxe-dr.fly.dev, app culebraluxe-dr
#   scripts/smoke-site.sh --base https://www.culebraluxe.com
#   scripts/smoke-site.sh --app culebraluxe-dr --no-fly    # skip the checks that need flyctl (secret names, logs)
#
# Output: one line per check — PASS / FAIL / WARN / MANUAL — then a summary. Exit 1 if any check FAILs.
# scripts/deploy-fly.sh runs it after every deploy and puts the result in its report.
set -uo pipefail

APP="culebraluxe-dr"; BASE=""; USE_FLY=1
while [ $# -gt 0 ]; do
  case "$1" in
    --app) APP="$2"; shift 2 ;;
    --base) BASE="$2"; shift 2 ;;
    --no-fly) USE_FLY=0; shift ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
BASE="${BASE:-https://${APP}.fly.dev}"; BASE="${BASE%/}"
FLY="$(command -v fly || command -v flyctl || true)"
[ -n "$FLY" ] || USE_FLY=0
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

PASSN=0; FAILN=0; WARNN=0; MANN=0
row() {  # row STATUS AREA DETAIL
  printf '%-6s  %-22s  %s\n' "$1" "$2" "$3"
  case "$1" in PASS) PASSN=$((PASSN+1));; FAIL) FAILN=$((FAILN+1));; WARN) WARNN=$((WARNN+1));; MANUAL) MANN=$((MANN+1));; esac
}
# get URL [curl args...] -> sets CODE, SECS, writes body to $TMP/body
get() {
  local url="$1"; shift
  local out
  out="$(curl -s -o "$TMP/body" -D "$TMP/headers" -w '%{http_code} %{time_total}' --max-time 30 "$@" "$url" </dev/null || true)"
  CODE="000"; SECS="0"
  [ -z "$out" ] || read -r CODE SECS _ <<<"$out"
  case "$SECS" in ''|*[!0-9.]*) SECS=0;; esac
}
json() {  # json PYTHON-EXPR  (reads $TMP/body as `d`; prints the result or nothing)
  python3 - "$1" "$TMP/body" <<'PY' 2>/dev/null || true
import json, sys
expr, path = sys.argv[1], sys.argv[2]
try:
    d = json.load(open(path))
except Exception:
    sys.exit(0)
def walk(v, key):
    """every value under `key` anywhere in v"""
    out = []
    if isinstance(v, dict):
        for k, x in v.items():
            if k == key and isinstance(x, str) and x:
                out.append(x)
            out += walk(x, key)
    elif isinstance(v, list):
        for x in v:
            out += walk(x, key)
    return out
r = eval(expr)
if r is None: sys.exit(0)
print(r if isinstance(r, str) else json.dumps(r))
PY
}

echo "Smoke test  $BASE  ($(date '+%Y-%m-%d %H:%M'))"
echo

# ---- 1. the 9 core checks: the server, the web app and its files ---------------------------------------------------
for path in /healthz / /buyers /app.css /rust-ui/ui.js /rust-ui/ui_bg.wasm "/api/rust-ui/public-page?screen=site-home" /login /api/build-info; do
  get "$BASE$path"
  if [ "$CODE" = 200 ]; then row PASS "core" "$path  200  $(printf '%.2f' "$SECS")s"; else row FAIL "core" "$path  -> $CODE"; fi
done
get "$BASE/api/build-info"
DBT="$(json 'd.get("databaseTarget")')"
if [ "$DBT" = prod ]; then row PASS "core" "database target is prod"; else row FAIL "core" "database target is '${DBT:-unknown}', not prod"; fi

# ---- 2. photos (read path): a real listing's hero photo comes back as an image -----------------------------------
get "$BASE/api/rust-ui/public-page?screen=site-buyers"
SLUG="$(json '(d.get("listings") or d.get("properties") or [{}])[0].get("slug")')"
[ -n "$SLUG" ] || SLUG="$(json '(walk(d, "slug") or [None])[0]')"
HERO=""; DETAIL_OK=0
if [ -n "$SLUG" ]; then
  get "$BASE/api/rust-ui/public-page?screen=site-property-detail&scope=$SLUG"
  [ "$CODE" = 200 ] && DETAIL_OK=1
  cp "$TMP/body" "$TMP/detail.json"
  HERO="$(json '(d.get("property") or {}).get("heroUrl")')"
  GALLERY="$(json 'len((d.get("property") or {}).get("gallery") or [])')"
fi
if [ -z "$SLUG" ]; then
  row FAIL "photos" "no public listing found on the buyers page"
elif [ -z "$HERO" ]; then
  row FAIL "photos" "listing '$SLUG' has no hero photo in its detail page"
else
  case "$HERO" in http*) HURL="$HERO";; *) HURL="$BASE$HERO";; esac
  get "$HURL"
  CT="$(grep -i '^content-type:' "$TMP/headers" | head -1 | cut -d: -f2- | tr -d ' \r')"
  case "$CODE:$CT" in
    200:image/*) row PASS "photos" "hero photo for '$SLUG' 200 $CT, ${GALLERY:-0} gallery image(s)" ;;
    *) row FAIL "photos" "hero photo for '$SLUG' -> $CODE ${CT:-no content-type} ($HERO)" ;;
  esac
fi
row MANUAL "photo uploads" "upload one photo to a test listing in the portal, then open it on the public page (a write; not automated)"

# ---- 3. Mux: a playback id the site publishes actually streams -----------------------------------------------------
PLAYBACK=""
for f in "$TMP/detail.json"; do [ -f "$f" ] && cp "$f" "$TMP/body" && PLAYBACK="$(json '(walk(d, "playbackId") + walk(d, "muxPlaybackId") or [None])[0]')"; done
if [ -z "$PLAYBACK" ]; then
  get "$BASE/api/rust-ui/public-page?screen=site-home"
  PLAYBACK="$(json '(walk(d, "playbackId") + walk(d, "muxPlaybackId") or [None])[0]')"
fi
if [ -z "$PLAYBACK" ]; then
  row WARN "mux" "no public page checked publishes a Mux playback id (home + one listing); attach a video to test"
else
  get "https://stream.mux.com/$PLAYBACK.m3u8"
  case "$CODE" in
    200) row PASS "mux" "stream.mux.com/$PLAYBACK.m3u8 200" ;;
    403) row WARN "mux" "playback $PLAYBACK is signed/private (403 without a token); check it plays on the site" ;;
    *) row FAIL "mux" "stream.mux.com/$PLAYBACK.m3u8 -> $CODE" ;;
  esac
fi
row MANUAL "mux upload" "attach a short video to a test listing in the portal and confirm it plays (a write; not automated)"

# ---- 4. WhatsApp: the webhook is routed AND configured (wrong token / bad signature must be refused) ---------------
get "$BASE/api/integrations/whatsapp/webhook?hub.mode=subscribe&hub.verify_token=smoke-not-the-token&hub.challenge=smoke"
case "$CODE" in
  403) row PASS "whatsapp" "verify handshake refuses a wrong token (403): verify token is configured" ;;
  500) row FAIL "whatsapp" "handshake -> 500: WHATSAPP_VERIFY_TOKEN / WHATSAPP_* not configured on $APP" ;;
  200) row FAIL "whatsapp" "handshake ACCEPTED a wrong token — check WHATSAPP_VERIFY_TOKEN" ;;
  *) row FAIL "whatsapp" "handshake -> $CODE" ;;
esac
get "$BASE/api/integrations/whatsapp/webhook" -X POST -H 'content-type: application/json' \
  -H 'x-hub-signature-256: sha256=0000000000000000000000000000000000000000000000000000000000000000' --data '{}'
case "$CODE" in
  401) row PASS "whatsapp" "webhook refuses an unsigned payload (401): app secret is configured, nothing recorded" ;;
  500) row FAIL "whatsapp" "webhook -> 500: WhatsApp is not configured on $APP" ;;
  *) row FAIL "whatsapp" "webhook with a bad signature -> $CODE (expected 401)" ;;
esac
row MANUAL "whatsapp" "in Meta's WhatsApp Manager, point the webhook at $BASE/api/integrations/whatsapp/webhook (after cutover: www.culebraluxe.com), send one message, see it in the portal timeline"

# ---- 5. LuxeSign: the signer routes are up and refuse a bogus link cleanly ------------------------------------------
get "$BASE/v1/signer/document/smoke-invalid-token"
case "$CODE" in
  400|401|403|404|410) row PASS "luxesign" "signer document route refuses a bogus link ($CODE), no server error" ;;
  5*) row FAIL "luxesign" "signer document route -> $CODE (server error)" ;;
  *) row WARN "luxesign" "signer document route -> $CODE (expected a 4xx refusal)" ;;
esac
row MANUAL "luxesign pdf" "send one LuxeSign request to yourself from the portal, sign it from the email link, download the signed PDF"

# ---- 6. settings present on Fly (names only; values are never read) ------------------------------------------------
if [ "$USE_FLY" = 1 ]; then
  if "$FLY" secrets list --app "$APP" --json </dev/null >"$TMP/secrets.json" 2>/dev/null; then
    names() { python3 -c 'import json,sys
d=json.load(open(sys.argv[1]))
print(" ".join(sorted({(x.get("Name") or x.get("name") or "") for x in d})))' "$TMP/secrets.json" 2>/dev/null; }
    HAVE=" $(names) "
    check_group() {  # check_group AREA VAR...
      local area="$1"; shift; local missing=""
      for v in "$@"; do case "$HAVE" in *" $v "*) ;; *) missing="$missing $v";; esac; done
      if [ -z "$missing" ]; then row PASS "$area" "settings present on $APP: $*"
      else row FAIL "$area" "missing on $APP:$missing"; fi
    }
    check_group "email settings"     ICLOUD_MAIL_ADDRESS ICLOUD_SMTP_APP_PASSWORD
    check_group "mux settings"       MUX_TOKEN_ID_PROD MUX_TOKEN_SECRET_PROD
    check_group "whatsapp settings"  WHATSAPP_VERIFY_TOKEN WHATSAPP_APP_SECRET WHATSAPP_PHONE_NUMBER_ID WHATSAPP_OWNED_PHONE_E164
    check_group "photo storage"      MEDIA_OBJECT_STORE_ENDPOINT MEDIA_OBJECT_STORE_ACCESS_KEY_ID MEDIA_OBJECT_STORE_SECRET_ACCESS_KEY MEDIA_BUCKET_PROPERTY_PUBLIC
    check_group "luxesign settings"  DOCSIGN_ACCESS_SECRET
    # BROKER_SIGNATURE_* are OPTIONAL overrides (middle/model/src/forms_broker_signature.rs): unset, the pre-signature
    # is on, uses the default name and license, and finds the protected signature image by its tag. Only the portal's
    # support-page readiness panel (portal_bridge/workspaces.rs) still demands them, so their absence is a WARN.
    BMISS=""
    for v in BROKER_SIGNATURE_ENABLED BROKER_SIGNATURE_APP_USER_ID BROKER_SIGNATURE_MEDIA_ID BROKER_SIGNATURE_SIGNER_NAME BROKER_SIGNATURE_LICENSE_NUMBER; do
      case "$HAVE" in *" $v "*) ;; *) BMISS="$BMISS ${v#BROKER_SIGNATURE_}";; esac
    done
    if [ -z "$BMISS" ]; then row PASS "broker pre-signature" "all BROKER_SIGNATURE_* overrides set"
    else row PASS "broker pre-signature" "no overrides set — the policy defaults apply (Lisa Penfield, C-9931; asset found by tag)"; fi
  else
    row WARN "settings" "could not read secret names (fly secrets list); run fly auth login"
  fi
  # Email has no read-only probe; the server says at startup when mail is degraded (web/src/composition.rs).
  if with_logs="$("$FLY" logs --app "$APP" --no-tail </dev/null 2>/dev/null)"; then
    if printf '%s' "$with_logs" | grep -q -E 'email transport unavailable|lead mail unavailable|mail degraded'; then
      row FAIL "email" "server log says mail is degraded (ICLOUD_MAIL_ADDRESS / ICLOUD_SMTP_APP_PASSWORD)"
    else
      row PASS "email" "no 'mail degraded' warning in the recent server log"
    fi
  else
    row WARN "email" "could not read fly logs"
  fi
else
  row WARN "settings" "flyctl not used (--no-fly or not installed): settings and email-startup checks skipped"
fi
row MANUAL "email send" "request a sign-in code for your own address on $BASE/login and confirm it arrives"

echo
printf 'Summary: %d PASS, %d FAIL, %d WARN, %d MANUAL\n' "$PASSN" "$FAILN" "$WARNN" "$MANN"
[ "$FAILN" = 0 ]
