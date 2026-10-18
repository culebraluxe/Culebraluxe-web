#!/usr/bin/env python3
"""orchestrate-status.py — the dynamic orchestrator's READ surface over the Forge control plane.

This is the prototype for "the orchestrator sits above Forge": it reads only, through the existing
read-only `forge sql` verb (`begin read only`; no writes are possible), and folds the live state into
one decision packet — work, next, holds, decisions. Writes stay on the canonical paths: a work item is
claimed by the Forge engine/worker, a HOLD is resolved through the HOLD path, a decision becomes a
`forge_decision` row or a packet/MEMORY entry. This script creates no queue and no state.

    scripts/orchestrate-status.sh                 # human summary
    scripts/orchestrate-status.sh --format json   # the decision packet as JSON

Requires the repo's `.env.local` (DATABASE_URL_PROD etc.) and the `cli` crate; `--target prod` is
explicit because a read that does not name its database is a read nobody can trust.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys

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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default="prod", choices=["prod", "dev"])
    parser.add_argument("--format", default="human", choices=["human", "json"])
    args = parser.parse_args()

    try:
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
