#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# RELEASE CI CHECK (FORGE-LOCAL-RELEASE-CI-CHECK-01) — a READ, never a build.
#
#   scripts/release-ci-check.sh <sha>
#
# WHY THIS EXISTS, and what it is NOT. A deploy job in CI was added and then deleted: it was inert (no
# token) but told the next reader how to switch it on, and the captain measured the cost — CI deploys were
# doubling the bill, about 30 builds a day where 1-2 were wanted. So CI now CHECKS and never SHIPS, and the
# deploy is local (`pnpm release`). That left a real gap: a red main could still be released by hand,
# because nothing in the release path read the gate. This closes it with a read.
#
# REFUSES on: a failed run, a pending run, no run at all for the sha, or a result that cannot be read. The
# last one matters most — "we could not check" and "it is fine" are different answers, and a release script
# that confuses them is worse than one that does not check.
#
# NAMED OPT-OUT, because a check that cannot be bypassed during an outage is a check somebody deletes:
#   RELEASE_CI_CHECK=skip     → allowed, loudly (this is not a pass, it is a decision)
#   RELEASE_CI_CMD="…"        → the reader to use (default: gh run list), so the rule is testable offline
# ---------------------------------------------------------------------------
set -uo pipefail

RELEASE_CI_CHECK="${RELEASE_CI_CHECK:-check}"
RELEASE_CI_CMD="${RELEASE_CI_CMD:-}"

sha="${1:-}"
if [ -z "$sha" ]; then
  printf 'release-ci-check: ERROR: a sha is required\n' >&2
  exit 2
fi

if [ "$RELEASE_CI_CHECK" = "skip" ]; then
  printf 'release-ci-check: SKIPPED by RELEASE_CI_CHECK=skip — the gate was NOT read (this is not a pass)\n'
  exit 0
fi

# The reader prints ONE line of `conclusion` values for the sha (gh: newest first), or nothing when there is
# no run. A reader that fails exits non-zero, which is "unreadable" and therefore a refusal.
read_conclusions() {
  if [ -n "$RELEASE_CI_CMD" ]; then
    eval "$RELEASE_CI_CMD" 2>/dev/null
  else
    gh run list --commit "$sha" --limit 5 --json conclusion --jq '.[].conclusion' 2>/dev/null
  fi
}

conclusions="$(read_conclusions)"
status=$?
if [ "$status" -ne 0 ]; then
  printf 'release-ci-check: REFUSED — could not read CI results for %s (unreadable is not clean)\n' "${sha:0:12}" >&2
  exit 1
fi

if [ -z "$(printf '%s' "$conclusions" | tr -d '[:space:]')" ]; then
  printf 'release-ci-check: REFUSED — no CI run exists for %s; refusing to release an unverified commit\n' "${sha:0:12}" >&2
  exit 1
fi

if printf '%s\n' "$conclusions" | grep -qiE 'failure|timed_out|cancelled|action_required|startup_failure'; then
  printf 'release-ci-check: REFUSED — CI is not green for %s:\n%s\n' "${sha:0:12}" "$conclusions" >&2
  exit 1
fi

if printf '%s\n' "$conclusions" | grep -qiE 'in_progress|queued|pending|waiting|requested'; then
  printf 'release-ci-check: REFUSED — CI is still running for %s:\n%s\n' "${sha:0:12}" "$conclusions" >&2
  exit 1
fi

# AN UNKNOWN STATE IS NOT A PASS. Everything is refused unless it is explicitly `success`, so a conclusion
# this script has never seen (a renamed or newly added state) cannot read as clean by accident — which is
# exactly the failure mode the whole "a skipped gate is never green" rule exists to prevent.
unknown="$(printf '%s\n' "$conclusions" | grep -viE '^[[:space:]]*success[[:space:]]*$' | head -3)"
if [ -n "$unknown" ]; then
  printf 'release-ci-check: REFUSED — unrecognised CI conclusion for %s: %s (an unknown state is not a pass)\n' \
    "${sha:0:12}" "$(printf '%s' "$unknown" | tr '\n' ' ')" >&2
  exit 1
fi

printf 'release-ci-check: green for %s (%s)\n' "${sha:0:12}" "$(printf '%s' "$conclusions" | tr '\n' ' ')"
exit 0
