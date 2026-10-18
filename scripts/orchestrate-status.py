#!/usr/bin/env python3
"""orchestrate-status.py — the dynamic orchestrator's READ surface over the Forge control plane.

This is the prototype for "the orchestrator sits above Forge": it reads only, through the existing
read-only `forge sql` verb (`begin read only`; no writes are possible), and folds the live state into
one decision packet — work, next, holds, decisions. Writes stay on the canonical paths: a work item is
claimed by the Forge engine/worker, a HOLD is resolved through the HOLD path, a decision becomes a
`forge_decision` row or a packet/MEMORY entry. This script creates no queue and no state.

    scripts/orchestrate-status.sh                 # human summary
    scripts/orchestrate-status.sh --format json   # the decision packet as JSON
    scripts/orchestrate-status.sh --digest        # one-page daily digest (stuck claims, HOLDs, failures)

The digest is one page and actionable. Its thresholds, defined here rather than implied:

  * STUCK — a `Claimed`/`Running` item the engine's own reaper would reap at the default window:
    `forge_reapable_claims(15)` (claim untouched for 15 minutes — `FORGE_DEFAULT_STALE_MINUTES` — or a
    worker marked `stale` in `forge_worker_health`). The digest reuses that rule; it does not invent a
    second definition of "stale".
  * FAILED — `agent_work_item.state = 'Error'` whose `finished_at` (or `updated_at`) is within 24 hours.
  * HOLDS — unresolved `forge_hold_record` rows, grouped by story and reason with repeat counts.

Requires the repo's `.env.local` (DATABASE_URL_PROD etc.) and the `cli` crate; `--target prod` is
explicit because a read that does not name its database is a read nobody can trust.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone

# The engine's default stale window (`FORGE_DEFAULT_STALE_MINUTES`, forge/src/engine/constants.rs), also
# the window the 15-minute stale sweep uses. A digest claim older than this is "stuck", not just slow.
STUCK_AFTER_MINUTES = 15
# Recent failures are a 24-hour window: yesterday's failure is history, an hour ago is today's problem.
FAILURE_WINDOW_HOURS = 24
# Caps that keep the digest one page; the totals are still printed so nothing is silently hidden.
MAX_STUCK_LINES = 5
MAX_HOLD_GROUPS = 10
MAX_FAILURE_LINES = 5
MAX_DECISION_LINES = 5
MAX_CANDIDATE_LINES = 3

QUERIES = {
    "work": """
        select id, story_id, state, priority, claimed_by, queued_at, updated_at
          from agent_work_item
         where state in ('Ready', 'Claimed', 'Running')
         order by priority desc, queued_at asc
    """,
    "holds": """
        select id, story_id, reason, originating_node, failure_class, resume_target, created_at
          from forge_hold_record
         where resolved_at is null
         order by created_at desc
    """,
    "decisions": """
        select key, statement, status, owner
          from forge_decision
         where status = 'active'
         order by key
    """,
    "contracts": """
        select story_id, node_id, decision, size, reason, updated_at
          from forge_role_contract
         where decision is not null
         order by updated_at desc
    """,
}

DIGEST_QUERIES = {
    # The canonical staleness rule, read from the function the engine itself uses (migration 275).
    "stuck": f"""
        select w.id, w.story_id, w.state, w.claimed_by, w.attempts, w.max_attempts,
               round(extract(epoch from now() - coalesce(w.heartbeat_at, w.updated_at)) / 60.0)::int
                   as age_minutes,
               r.why
          from forge_reapable_claims({STUCK_AFTER_MINUTES}) r
          join agent_work_item w on w.id = r.id
         where w.state in ('Claimed', 'Running')
         order by coalesce(w.heartbeat_at, w.updated_at) asc
    """,
    # Grouped rows, not one line per HOLD: the repeated reason is the signal, the count is the weight.
    "hold_groups": """
        select story_id, reason,
               count(*) as hold_count,
               min(failure_class) as failure_class,
               min(resume_target) as resume_target,
               round(extract(epoch from now() - min(created_at)))::bigint as oldest_age_seconds
          from forge_hold_record
         where resolved_at is null
         group by story_id, reason
         order by min(created_at) asc
    """,
    "failures": f"""
        select id, story_id, state, attempts, max_attempts, error_text, finished_at, updated_at
          from agent_work_item
         where state = 'Error'
           and coalesce(finished_at, updated_at) >= now() - interval '{FAILURE_WINDOW_HOURS} hours'
         order by coalesce(finished_at, updated_at) desc
    """,
    "work_queue": """
        select state, count(*) as items
          from forge_work_queue
         where state in ('Pending', 'Running')
         group by state
    """,
    "runtime": """
        select paused, updated_by
          from forge_runtime_control
         where id = 1
    """,
    "decisions": """
        select key, statement, owner, promoted_at
          from forge_decision
         where status = 'active'
         order by promoted_at desc nulls last, key
    """,
    "candidates": """
        select key, statement, created_at
          from forge_decision
         where status = 'candidate'
         order by created_at desc
    """,
}


class StatusError(RuntimeError):
    pass


def forge_sql(sql: str, target: str) -> list[dict]:
    result = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "cli",
            "--",
            "forge",
            "sql",
            "--target",
            target,
            "--format",
            "json",
            "--sql",
            " ".join(sql.split()),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise StatusError(f"forge sql failed (exit {result.returncode}): {result.stderr.strip()[:400]}")
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise StatusError(f"forge sql returned non-JSON: {error}: {result.stdout[:200]!r}") from error
    return payload.get("result", [])


def build_packet(target: str) -> dict:
    rows = {name: forge_sql(sql, target) for name, sql in QUERIES.items()}
    work = rows["work"]
    ready = [item for item in work if item.get("state") == "Ready"]
    active = [item for item in work if item.get("state") in ("Claimed", "Running")]
    holds = rows["holds"]
    decisions = rows["decisions"]
    next_item = ready[0] if ready else None
    blocked = bool(holds) or bool(decisions and any(d.get("status") == "active" for d in decisions))
    return {
        "target": target,
        "read_only": True,
        "work": {"ready": len(ready), "active": len(active), "items": work},
        "next": next_item,
        "holds": holds,
        "blocked": blocked,
        "active_decisions": decisions,
        "latest_contracts": rows["contracts"],
        # A dynamic orchestrator decides from these facts; it never writes them here.
        "decision_required": bool(holds) or next_item is None,
    }


def build_digest(target: str) -> dict:
    """The one-page daily digest: only the facts that change today's action."""
    rows = {name: forge_sql(sql, target) for name, sql in DIGEST_QUERIES.items()}
    work = forge_sql(QUERIES["work"], target)
    ready = [item for item in work if item.get("state") == "Ready"]
    active = [item for item in work if item.get("state") in ("Claimed", "Running")]
    queue = {row.get("state"): int(row.get("items") or 0) for row in rows["work_queue"]}
    return {
        "target": target,
        "read_only": True,
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "work": {"ready": len(ready), "active": len(active), "items": work},
        "next": ready[0] if ready else None,
        "stuck": rows["stuck"],
        "holds": rows["hold_groups"],
        "failures": rows["failures"],
        "work_queue": {"pending": queue.get("Pending", 0), "running": queue.get("Running", 0)},
        "runtime": rows["runtime"][0] if rows["runtime"] else None,
        "decisions": rows["decisions"],
        "candidates": rows["candidates"],
    }


def _short(text: object, limit: int) -> str:
    """One line, bounded — a digest line is not a log dump."""
    if text is None:
        return ""
    line = str(text).strip().splitlines()[0].strip() if str(text).strip() else ""
    if len(line) <= limit:
        return line
    return line[: max(limit - 1, 0)].rstrip() + "…"


def _age(seconds: object) -> str:
    """Seconds -> the shortest honest age (15m, 3h, 2d)."""
    if seconds is None:
        return "?"
    seconds = max(int(seconds), 0)
    if seconds < 60:
        return f"{seconds}s"
    minutes = seconds // 60
    if minutes < 60:
        return f"{minutes}m"
    hours = minutes // 60
    if hours < 48:
        return f"{hours}h"
    return f"{hours // 24}d"


def _digest_action(digest: dict) -> str:
    """The single next thing a human/orchestrator should do, or an explicit "nothing"."""
    work = digest["work"]
    runtime = digest["runtime"]
    if runtime and runtime.get("paused"):
        paused_by = runtime.get("updated_by") or "unknown"
        return (
            f"claims are PAUSED by {paused_by} — clear forge_runtime_control.paused before "
            "expecting work to move"
        )
    if digest["stuck"]:
        return (
            f"inspect {len(digest['stuck'])} stuck claim(s) — reclaim/requeue through the engine's "
            "canonical stale path, not by hand"
        )
    if digest["holds"]:
        total = sum(int(group.get("hold_count") or 0) for group in digest["holds"])
        oldest = digest["holds"][0].get("story_id")
        return f"resolve {total} open HOLD(s) starting with {oldest} — decisions unblock, more work does not"
    if digest["failures"]:
        return (
            f"review {len(digest['failures'])} work item(s) that errored in the last "
            f"{FAILURE_WINDOW_HOURS}h before re-queueing anything"
        )
    if digest["next"]:
        return (
            f"dispatch {digest['next'].get('story_id')} through the existing engine/worker "
            "(no manual hand-claim)"
        )
    if digest["candidates"]:
        return f"promote or decline {len(digest['candidates'])} candidate decision(s)"
    if work["active"]:
        return f"{work['active']} item(s) in flight — let them settle; nothing to dispatch"
    return "nothing to dispatch, no open HOLDs, no recent failures — stay idle; do not invent work"


def print_digest(digest: dict) -> None:
    work = digest["work"]
    queue = digest["work_queue"]
    runtime = digest["runtime"]
    if runtime is None:
        claims = "claims unknown (no runtime-control row)"
    elif runtime.get("paused"):
        claims = f"claims PAUSED by {runtime.get('updated_by') or 'unknown'}"
    else:
        claims = "claims open"

    print(f"FORGE DAILY DIGEST  (target={digest['target']}, read-only)  {digest['generated_at']}")
    print("=" * 76)
    print(
        f"QUEUE     ready {work['ready']} · active {work['active']} · {claims} · "
        f"work-queue {queue['pending']} pending / {queue['running']} running"
    )

    stuck = digest["stuck"]
    if stuck:
        print(f"STUCK     {len(stuck)} claimed/running at or past {STUCK_AFTER_MINUTES}m (or stale worker)")
        for item in stuck[:MAX_STUCK_LINES]:
            print(
                f"  {item.get('story_id')}  {item.get('state')}  "
                f"claim={item.get('claimed_by') or '-'}  "
                f"{item.get('attempts')}/{item.get('max_attempts')} attempts  "
                f"idle {item.get('age_minutes')}m  {_short(item.get('why'), 60)}"
            )
        if len(stuck) > MAX_STUCK_LINES:
            print(f"  … {len(stuck) - MAX_STUCK_LINES} more stuck")
    else:
        print(f"STUCK     none — no claim older than {STUCK_AFTER_MINUTES}m and no stale worker")

    next_item = digest["next"]
    if next_item:
        print(
            f"NEXT      {next_item.get('story_id')} (priority {next_item.get('priority')}) — "
            "dispatch through the existing engine/worker"
        )
    elif work["active"]:
        print(f"NEXT      none ready — {work['active']} item(s) in flight; nothing to dispatch")
    else:
        print("NEXT      none ready — the queue is empty")

    holds = digest["holds"]
    if holds:
        stories = {group.get("story_id") for group in holds}
        total = sum(int(group.get("hold_count") or 0) for group in holds)
        print(f"HOLDS     {total} open across {len(stories)} stories")
        for group in holds[:MAX_HOLD_GROUPS]:
            resume = group.get("resume_target") or "no resume target"
            failure_class = group.get("failure_class")
            suffix = f"  [{failure_class}]" if failure_class else ""
            print(
                f"  {group.get('story_id')}  {group.get('hold_count')}×  "
                f"{_short(group.get('reason'), 90)}  -> {resume}  "
                f"(oldest {_age(group.get('oldest_age_seconds'))}){suffix}"
            )
        if len(holds) > MAX_HOLD_GROUPS:
            print(f"  … {len(holds) - MAX_HOLD_GROUPS} more HOLD groups")
    else:
        print("HOLDS     none open")

    failures = digest["failures"]
    if failures:
        print(f"FAILED    {len(failures)} work item(s) errored in the last {FAILURE_WINDOW_HOURS}h")
        for item in failures[:MAX_FAILURE_LINES]:
            print(
                f"  {item.get('story_id')}  {item.get('attempts')}/{item.get('max_attempts')} attempts  "
                f"{_short(item.get('error_text') or 'no error text', 100)}"
            )
        if len(failures) > MAX_FAILURE_LINES:
            print(f"  … {len(failures) - MAX_FAILURE_LINES} more failed")
    else:
        print(f"FAILED    none in the last {FAILURE_WINDOW_HOURS}h")

    decisions = digest["decisions"]
    if decisions:
        print(f"DECISIONS {len(decisions)} active")
        for decision in decisions[:MAX_DECISION_LINES]:
            print(f"  {decision.get('key')} — {_short(decision.get('statement'), 110)}")
        if len(decisions) > MAX_DECISION_LINES:
            print(f"  … {len(decisions) - MAX_DECISION_LINES} more active")
    else:
        print("DECISIONS none active")

    candidates = digest["candidates"]
    if candidates:
        print(f"CANDIDATES {len(candidates)} awaiting promotion")
        for candidate in candidates[:MAX_CANDIDATE_LINES]:
            print(f"  {candidate.get('key')} — {_short(candidate.get('statement'), 110)}")
        if len(candidates) > MAX_CANDIDATE_LINES:
            print(f"  … {len(candidates) - MAX_CANDIDATE_LINES} more candidates")

    print(f"ACTION    {_digest_action(digest)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default="prod", choices=["prod", "dev"])
    parser.add_argument("--format", default="human", choices=["human", "json"])
    parser.add_argument(
        "--digest",
        action="store_true",
        help="one-page daily digest: stuck claims, grouped HOLDs, failures in 24h, queue, decisions",
    )
    args = parser.parse_args()

    if args.digest and args.format != "human":
        print(
            "FAIL  --digest is the human digest; it cannot be combined with --format json",
            file=sys.stderr,
        )
        return 2

    try:
        if args.digest:
            print_digest(build_digest(args.target))
            return 0
        packet = build_packet(args.target)
    except StatusError as error:
        print(f"FAIL  {error}", file=sys.stderr)
        return 1

    if args.format == "json":
        print(json.dumps(packet, indent=2))
        return 0

    work = packet["work"]
    print(f"FORGE ORCHESTRATOR STATUS (target={packet['target']}, read-only)")
    print(f"  WORK     ready {work['ready']} · active {work['active']}")
    for item in work["items"][:8]:
        print(f"    {item.get('state'):8} {item.get('story_id')}  priority={item.get('priority')}")
    if packet["next"]:
        print(f"  NEXT     {packet['next'].get('story_id')} (priority {packet['next'].get('priority')})")
    else:
        print("  NEXT     none ready — the queue is empty or everything is blocked")
    if packet["holds"]:
        print(f"  HOLDS    {len(packet['holds'])} open")
        for hold in packet["holds"][:5]:
            print(f"    {hold.get('story_id')}  {hold.get('reason')}  -> {hold.get('resume_target')}")
    else:
        print("  HOLDS    none open")
    if packet["active_decisions"]:
        print(f"  DECISIONS {len(packet['active_decisions'])} active")
        for decision in packet["active_decisions"][:5]:
            print(f"    {decision.get('key')}  {decision.get('statement')}")
    else:
        print("  DECISIONS none active")
    print(f"  DECISION REQUIRED  {str(packet['decision_required']).lower()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
