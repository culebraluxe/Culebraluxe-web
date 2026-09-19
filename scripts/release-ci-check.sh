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
  printf 'release-ci-check: REFUSED — could not read CI results for %s (unreadable is not clean)\n' "${sha:0:12}" >&2
  exit 1
fi

verdict="$(
  printf '%s' "$runs" | RELEASE_CI_WORKFLOW="$RELEASE_CI_WORKFLOW" RELEASE_CI_SHA="$sha" node -e '
    let raw = "";
    process.stdin.on("data", (chunk) => (raw += chunk));
    process.stdin.on("end", () => {
      const out = (line) => { process.stdout.write(line); process.exit(0); };
      const fail = (line) => { process.stderr.write(line); process.exit(1); };
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
        return fail("release-ci-check: REFUSED — the required workflow " + required + " has no run for " + sha.slice(0, 12) + "; an unrelated workflow success does not authorise a release\n");
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
  printf '%s' "$verdict" >&2
  exit 1
fi
printf '%s' "$verdict"
exit 0
