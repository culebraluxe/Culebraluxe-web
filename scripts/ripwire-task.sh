#!/usr/bin/env bash
# Task-scoped ripwire query over the APPLICATION tree.
#
#   pnpm rw:for "the services composition root and the MVI page controller"
#
# Why this exists instead of a one-liner in package.json:
#   - ripwire accepts the concept as --for=VALUE, never as "--for VALUE", so the
#     query has to be concatenated onto the flag inside a shell — which means
#     nested quoting, which JSON escape sequences make unreadable and easy to get
#     wrong. A script file keeps it plain.
#   - The roots are listed explicitly rather than using ".", because ripwire does
#     NOT honor .gitignore. A bare `ripwire .` on this repo indexes .next build
#     chunks and the nested gsd-core / praxis / claude-orchestrate reference
#     clones, and those dominate the god-files and modules lists until the map is
#     useless for app work (measured: 9,482 files vs 2,824 with the excludes).
#
# `pnpm rw:map` is the whole-repo variant of the same scope.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: pnpm rw:for \"<the task in your own words>\"" >&2
  exit 1
fi

exec ripwire \
  app components lib db services ui workflow_app testv2 scripts \
  --for="$*"
