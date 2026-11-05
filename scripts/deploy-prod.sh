#!/usr/bin/env bash
# PRODUCTION DEPLOY — the name every script, doc and habit already uses, now pointing at Fly.
#
# The Vercel deploy this file used to implement is retired (W2.4/W2.5). It lives, verbatim, in
# `devops/break-glass/vercel/` because the Captain asked for it to survive as an emergency exit — read that
# directory's README before considering it, and expect to rebuild its environment from scratch.
#
# Production is `culebraluxe-dr` on Fly, deployed by `scripts/deploy-fly.sh`. This shim keeps
# `pnpm deploy:prod`, a launchd job, or a hand-typed `scripts/deploy-prod.sh` meaning "deploy production"
# instead of failing with "no such file".
#
#   scripts/deploy-prod.sh                  # the Fly deploy, as deploy-fly.sh runs it
#   scripts/deploy-prod.sh --source scion   # flags pass straight through
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
printf 'deploy-prod.sh is the Fly deploy now; running scripts/deploy-fly.sh.\n' >&2
exec "$ROOT_DIR/scripts/deploy-fly.sh" "$@"
