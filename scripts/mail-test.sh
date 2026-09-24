#!/usr/bin/env bash
# Send email as the business address, using the mail settings in .env.local.
#   pnpm mail:test you@example.com      one plain test email
#   pnpm mail:preview you@example.com   the two website-lead emails, filled with a sample lead
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
[ -f .env.local ] || { echo "missing .env.local" >&2; exit 2; }
# Export only the mail settings; nothing else from .env.local reaches the process.
while IFS= read -r line; do
  case "$line" in
    ICLOUD_MAIL_ADDRESS=*|ICLOUD_MAIL_USERNAME=*|ICLOUD_SMTP_APP_PASSWORD=*|ICLOUD_SMTP_HOST=*|ICLOUD_SMTP_PORT=*|MAIL_FROM_NAME=*|LEAD_NOTIFY_ADDRESS=*|PUBLIC_SITE_URL=*)
      export "${line?}" ;;
  esac
done < .env.local
if [ "${1:-}" = "--preview" ]; then
  shift
  exec cargo run --quiet --manifest-path rust/Cargo.toml -p server --bin lead_mail_preview -- "$@"
fi
exec cargo run --quiet --manifest-path rust/Cargo.toml -p integrations --bin mail-test -- "$@"
