#!/usr/bin/env python3
"""Compare the active OpenCode model list with config/model-roster.json.

Read-only. Exit 0 when they agree, 1 on drift, 2 when `opencode models` cannot run.
`--report-only` always exits 0 (for a digest/alerts pulse).
"""
import argparse, json, shutil, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROSTER = ROOT / "config" / "model-roster.json"


def active_models() -> list[str]:
    if not shutil.which("opencode"):
        raise RuntimeError("opencode CLI not found on PATH")
    out = subprocess.run(["opencode", "models"], capture_output=True, text=True, timeout=60)
    if out.returncode != 0:
        raise RuntimeError(f"opencode models failed: {out.stderr.strip() or out.stdout.strip()}")
    return [line.strip() for line in out.stdout.splitlines() if line.strip() and not line.startswith("#")]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--report-only", action="store_true")
    args = ap.parse_args()
    roster = json.loads(ROSTER.read_text())
    try:
        active = set(active_models())
    except Exception as error:
        print(f"model-roster-sync: {error}", file=sys.stderr)
        return 0 if args.report_only else 2
    registry = set(roster["paths"]["opencode"]["models"])
    added = sorted(active - registry)
    removed = sorted(registry - active)
    retired = {entry["id"] for entry in roster.get("retired", [])}
    result = {
        "registry": len(registry),
        "active": len(active),
        "added": added,
        "removed": removed,
        "unregistered_new": [m for m in added if m not in retired],
    }
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"opencode: {len(active)} active, {len(registry)} registered")
        for m in added:
            print(f"  NEW     {m}")
        for m in removed:
            print(f"  MISSING {m}")
        if not added and not removed:
            print("  in sync")
    drift = bool(added or removed)
    return 0 if args.report_only or not drift else 1


if __name__ == "__main__":
    raise SystemExit(main())
