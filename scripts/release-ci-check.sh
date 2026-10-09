#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# RELEASE CI CHECK (FORGE-LOCAL-RELEASE-CI-CHECK-01) — a READ, never a build.
#
#   scripts/release-ci-check.sh <sha>
#
# WHY IT READS CI AT ALL, and what it is NOT. A deploy job in CI was added and then deleted: it was inert
# (no token) but told the next reader how to switch it on, and the captain measured the cost — CI deploys
# were doubling the bill, about 30 builds a day where 1-2 were wanted. So CI now CHECKS and never SHIPS,
# and the deploy is local (`pnpm release`).
#
# THIS IS A REPORT, NOT A GUN (captain's ruling, 2026-10-08): "tests should be my choice to run, not a gun
# to my head to do a deploy." The default mode now PRINTS the verdict for the exact sha it was asked
# about — green, red, pending, no run, or unreadable — and lets the release continue on every one of them,
# including the ugly ones. What ships is decided by the build and the live probe below, which measure the
# artifact itself; a test's verdict on some other question never stood between the captain and a deploy.
#
# `require` remains, as a NAMED opt-in rather than a default: release-day discipline, asked for out loud.
#
#   RELEASE_CI_CHECK=read     → (DEFAULT) read and print the verdict; never refuse, never silent
#   RELEASE_CI_CHECK=require  → a failed, pending, missing or unreadable run stops the release
#   RELEASE_CI_CHECK=skip     → do not read CI at all (says so loudly; this is not a pass)
#   RELEASE_CI_CMD="…"        → the reader to use (default: gh run list), so the rule is testable offline
#
# "We could not read it" and "it is green" stay different ANSWERS in either mode — only `require` makes
# the difference a refusal. An unknown mode is reported and treated as `read`, because a typo must not
# acquire the power to stop a deploy that the default does not have.
# ---------------------------------------------------------------------------
set -uo pipefail

RELEASE_CI_CHECK="$(printf '%s' "${RELEASE_CI_CHECK:-read}" | tr '[:upper:]' '[:lower:]')"
case "$RELEASE_CI_CHECK" in
  read|require|skip) ;;
  *)
    printf 'release-ci-check: WARNING — unknown RELEASE_CI_CHECK="%s" (read|require|skip); treating as read\n' \
      "$RELEASE_CI_CHECK" >&2
    RELEASE_CI_CHECK="read"
    ;;
esac
case "$RELEASE_CI_CHECK" in
  require)
    printf 'release-ci-check: MODE require — a non-green run WILL stop this release (asked for out loud)\n'
    ;;
  skip)
    printf 'release-ci-check: MODE skip — CI will not be read at all (this is not a pass, it is a decision)\n'
    ;;
  *)
    printf 'release-ci-check: MODE read — the verdict below is information; it does not stop the release\n'
    ;;
esac
RELEASE_CI_WORKFLOW="${RELEASE_CI_WORKFLOW:-gates.yml}"

sha="${1:-}"
if [ -z "$sha" ]; then
  printf 'release-ci-check: ERROR: a sha is required\n' >&2
  exit 2
fi

if [ "$RELEASE_CI_CHECK" = "skip" ]; then
  printf 'release-ci-check: SKIPPED by RELEASE_CI_CHECK=skip — the gate was NOT read (this is not a pass)\n'
  exit 0
fi

read_conclusions() {
  if [ -n "$RELEASE_CI_CMD" ]; then
    eval "$RELEASE_CI_CMD" 2>/dev/null
  else
    gh run list --commit "$sha" --limit 30 \
      --json databaseId,name,workflowName,headSha,status,conclusion,attempt,createdAt,event 2>/dev/null
  fi
}

runs="$(read_conclusions)"
status=$?
if [ "$status" -ne 0 ]; then
  # "Unreadable is not clean" stays true in both modes; only `require` turns that difference into a refusal.
  if [ "$RELEASE_CI_CHECK" = "require" ]; then
    printf 'release-ci-check: REFUSED — could not read CI results for %s (unreadable is not clean)\n' "${sha:0:12}" >&2
    exit 1
  fi
  printf 'release-ci-check: UNREADABLE — could not read CI results for %s (informational: this does not stop the release)\n' "${sha:0:12}"
  exit 0
fi

verdict="$(
  printf '%s' "$runs" | RELEASE_CI_WORKFLOW="$RELEASE_CI_WORKFLOW" RELEASE_CI_SHA="$sha" \
    RELEASE_CI_MODE="$RELEASE_CI_CHECK" node -e '
    let raw = "";
    process.stdin.on("data", (chunk) => (raw += chunk));
    process.stdin.on("end", () => {
      const out = (line) => { process.stdout.write(line); process.exit(0); };
      const strict = String(process.env.RELEASE_CI_MODE || "read").toLowerCase() === "require";
      // REFUSE only when asked to (RELEASE_CI_CHECK=require). Otherwise the SAME finding is printed with the
      // "REFUSED" wording stripped and the sentence that says what it is: information. One classifier, so
      // "read" and "require" can never disagree about what CI said — only about what is done with it.
      const fail = (line) => {
        if (strict) { process.stderr.write(line); process.exit(1); }
        const finding = line.replace(/\n$/, "").replace("release-ci-check: REFUSED — ", "release-ci-check: ");
        process.stdout.write(finding + " (informational: this does not stop the release)\n");
        process.exit(0);
      };
      let runs;
      try {
        runs = JSON.parse(raw);
      } catch {
        return fail("release-ci-check: REFUSED — the CI provider returned something that is not a run list (malformed)\n");
      }
      if (!Array.isArray(runs)) {
        return fail("release-ci-check: REFUSED — the CI provider did not return a list of runs (malformed)\n");
      }
      const sha = String(process.env.RELEASE_CI_SHA || "").toLowerCase();
      const required = String(process.env.RELEASE_CI_WORKFLOW || "").toLowerCase();
      const matchesRequired = (run) =>
        [run && run.name, run && run.workflowName]
          .filter((value) => typeof value === "string")
          .some((value) => {
            const lower = value.toLowerCase();
            return lower === required || lower === required.replace(/\.ya?ml$/, "") || lower.endsWith("/" + required);
          });
      const forSha = runs.filter((run) => matchesRequired(run) && String((run && run.headSha) || "").toLowerCase() === sha);
      const requiredRuns = runs.filter((run) => matchesRequired(run));
      if (forSha.length === 0 && requiredRuns.length > 0) {
        return fail("release-ci-check: REFUSED — no run of the required workflow " + required + " exists for " + sha.slice(0, 12) + " (found runs for other SHAs)\n");
      }
      if (forSha.length === 0) {
        return fail("release-ci-check: REFUSED — the required workflow " + required + " has no run for " + sha.slice(0, 12) + " (an unrelated workflow success is not a substitute)\n");
      }
      const latest = forSha
        .slice()
        .sort((a, b) => Number((b && b.databaseId) || 0) - Number((a && a.databaseId) || 0) || Number((b && b.attempt) || 0) - Number((a && a.attempt) || 0))[0];
      const where = "run " + latest.databaseId + " attempt " + (latest.attempt || 1) + " (" + latest.status + "/" + (latest.conclusion || "") + ")";
      if (latest.status !== "completed") {
        return fail("release-ci-check: REFUSED — the required workflow " + required + " is not finished for " + sha.slice(0, 12) + ": " + where + "\n");
      }
      if (latest.conclusion !== "success") {
        return fail("release-ci-check: REFUSED — the required workflow " + required + " did not succeed for " + sha.slice(0, 12) + ": " + where + "\n");
      }
      return out("release-ci-check: green — " + required + " completed successfully for " + sha.slice(0, 12) + " (" + where + ")\n");
    });
  '
)"
verdict_rc=$?

if [ "$verdict_rc" -ne 0 ]; then
  printf '%s\n' "$verdict" >&2
  exit 1
fi
# A newline, because `$( )` stripped the verdict's own: without it the release log glues the verdict to the
# next `--- release gate exit: N ---` line and a reader has to guess where one ended.
printf '%s\n' "$verdict"
exit 0
