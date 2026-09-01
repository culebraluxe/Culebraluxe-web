#!/usr/bin/env bash

set -euo pipefail

DEFAULT_FROM_PHONE="16172516169"
DEFAULT_WEBHOOK_URL="https://culebraluxe.com/api/integrations/whatsapp/webhook"

FROM_PHONE="$DEFAULT_FROM_PHONE"
WEBHOOK_URL="${WHATSAPP_WEBHOOK_URL:-$DEFAULT_WEBHOOK_URL}"
ENV_FILE=""
MODE="post"

usage() {
  cat <<'USAGE'
Send a signed synthetic inbound message to the CulebraLuxe WhatsApp webhook.

Usage:
  pnpm whatsapp:webhook:test [options]

Options:
  --from PHONE       Sender phone. Defaults to +1 617-251-6169.
                     A 10-digit US number is automatically prefixed with 1.
  --env-file FILE    Override the env file. Defaults to .env.local when present.
  --url URL          Override the production webhook URL.
  --handshake        Test only the Meta verification handshake.
  -h, --help         Show this help.

Required for a message POST:
  WHATSAPP_APP_SECRET
  WHATSAPP_PHONE_NUMBER_ID
  WHATSAPP_OWNED_PHONE_E164

Required for --handshake:
  WHATSAPP_VERIFY_TOKEN

Missing values are prompted for when the script runs in a terminal. Secrets are
never written by this script or printed to the screen.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --from)
      [[ $# -ge 2 ]] || { echo "--from requires a phone number." >&2; exit 2; }
      FROM_PHONE="$2"
      shift 2
      ;;
    --env-file)
      [[ $# -ge 2 ]] || { echo "--env-file requires a path." >&2; exit 2; }
      ENV_FILE="$2"
      shift 2
      ;;
    --url)
      [[ $# -ge 2 ]] || { echo "--url requires a URL." >&2; exit 2; }
      WEBHOOK_URL="$2"
      shift 2
      ;;
    --handshake)
      MODE="handshake"
      shift
      ;;
    --)
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$ENV_FILE" && -f ".env.local" ]]; then
  ENV_FILE=".env.local"
fi

if [[ -n "$ENV_FILE" ]]; then
  [[ -f "$ENV_FILE" ]] || { echo "Env file not found: $ENV_FILE" >&2; exit 2; }
  command -v node >/dev/null 2>&1 || {
    echo "Node.js is required to read $ENV_FILE safely." >&2
    exit 2
  }

  load_env_file_value() {
    local variable_name="$1"
    local current_value="${!variable_name:-}"
    local file_value

    # Explicit shell values win. Otherwise use Node's dotenv parser so values
    # containing $, spaces, or shell metacharacters are never evaluated.
    [[ -n "$current_value" ]] && return
    file_value="$(node --env-file="$ENV_FILE" \
      -e 'process.stdout.write(process.env[process.argv[1]] || "")' \
      "$variable_name")"
    printf -v "$variable_name" '%s' "$file_value"
  }

  load_env_file_value WHATSAPP_APP_SECRET
  load_env_file_value WHATSAPP_PHONE_NUMBER_ID
  load_env_file_value WHATSAPP_OWNED_PHONE_E164
  load_env_file_value WHATSAPP_VERIFY_TOKEN
fi

prompt_value() {
  local variable_name="$1"
  local label="$2"
  local secret="${3:-false}"
  local current_value="${!variable_name:-}"
  local entered_value

  [[ -n "$current_value" ]] && return
  if [[ ! -t 0 ]]; then
    echo "Missing required environment variable: $variable_name" >&2
    exit 2
  fi

  if [[ "$secret" == "true" ]]; then
    read -r -s -p "$label: " entered_value
    echo
  else
    read -r -p "$label: " entered_value
  fi
  [[ -n "$entered_value" ]] || { echo "$variable_name cannot be empty." >&2; exit 2; }
  printf -v "$variable_name" '%s' "$entered_value"
}

if [[ "$MODE" == "handshake" ]]; then
  prompt_value WHATSAPP_VERIFY_TOKEN "Production WhatsApp verify token" true
  CHALLENGE="culebraluxe-fixture-$(date +%s)"
  RESPONSE="$(curl -sS --get "$WEBHOOK_URL" \
    --data-urlencode "hub.mode=subscribe" \
    --data-urlencode "hub.verify_token=$WHATSAPP_VERIFY_TOKEN" \
    --data-urlencode "hub.challenge=$CHALLENGE")"
  if [[ "$RESPONSE" != "$CHALLENGE" ]]; then
    echo "Handshake failed. Response: $RESPONSE" >&2
    exit 1
  fi
  echo "Handshake passed: $RESPONSE"
  exit 0
fi

prompt_value WHATSAPP_APP_SECRET "Production Meta app secret" true
prompt_value WHATSAPP_PHONE_NUMBER_ID "Production WhatsApp phone-number ID"
prompt_value WHATSAPP_OWNED_PHONE_E164 "Owned WhatsApp business number (E.164)"

FROM_DIGITS="$(printf '%s' "$FROM_PHONE" | tr -cd '[:digit:]')"
if [[ ${#FROM_DIGITS} -eq 10 ]]; then
  FROM_DIGITS="1${FROM_DIGITS}"
fi
if [[ ! "$FROM_DIGITS" =~ ^[1-9][0-9]{9,14}$ ]]; then
  echo "Invalid sender phone: $FROM_PHONE" >&2
  exit 2
fi

OWNED_DIGITS="$(printf '%s' "$WHATSAPP_OWNED_PHONE_E164" | tr -cd '[:digit:]')"
if [[ ! "$OWNED_DIGITS" =~ ^[1-9][0-9]{9,14}$ ]]; then
  echo "Invalid WHATSAPP_OWNED_PHONE_E164: use E.164, such as +17875551212." >&2
  exit 2
fi
if [[ ! "$WHATSAPP_PHONE_NUMBER_ID" =~ ^[0-9]+$ ]]; then
  echo "WHATSAPP_PHONE_NUMBER_ID must contain only digits." >&2
  exit 2
fi

NOW="$(date +%s)"
MESSAGE_ID="wamid.CULEBRALUXE_FIXTURE.${NOW}.${RANDOM}"
BODY="$(printf '%s' '{"object":"whatsapp_business_account","entry":[{"id":"WABA_FIXTURE","changes":[{"field":"messages","value":{"messaging_product":"whatsapp","metadata":{"display_phone_number":"'"$OWNED_DIGITS"'","phone_number_id":"'"$WHATSAPP_PHONE_NUMBER_ID"'"},"contacts":[{"profile":{"name":"CulebraLuxe Fixture"},"wa_id":"'"$FROM_DIGITS"'"}],"messages":[{"from":"'"$FROM_DIGITS"'","id":"'"$MESSAGE_ID"'","timestamp":"'"$NOW"'","type":"text","text":{"body":"synthetic inbound from fixture"}}]}}]}]}')"

SIGNATURE="$(printf '%s' "$BODY" | openssl dgst -sha256 -hmac "$WHATSAPP_APP_SECRET" | awk '{print $NF}')"
RESPONSE_FILE="$(mktemp "${TMPDIR:-/tmp}/culebraluxe-wa-fixture.XXXXXX")"
trap 'rm -f "$RESPONSE_FILE"' EXIT

echo "Sending synthetic inbound WhatsApp event"
echo "  From: +$FROM_DIGITS"
echo "  To:   +$OWNED_DIGITS"
echo "  ID:   $MESSAGE_ID"
echo "  URL:  $WEBHOOK_URL"

HTTP_STATUS="$(curl -sS -o "$RESPONSE_FILE" -w '%{http_code}' \
  -X POST "$WEBHOOK_URL" \
  -H "Content-Type: application/json" \
  -H "X-Hub-Signature-256: sha256=$SIGNATURE" \
  --data-binary "$BODY")"

echo "HTTP $HTTP_STATUS"
cat "$RESPONSE_FILE"
echo

if [[ "$HTTP_STATUS" != "200" ]]; then
  exit 1
fi
