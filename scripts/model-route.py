#!/usr/bin/env python3
"""Validate/model-route against the provider-path registry. No secrets, no network."""
import argparse, json, sys, pathlib
ROOT = pathlib.Path(__file__).resolve().parent.parent
REG = json.loads((ROOT / "config/model-roster.json").read_text())
def find(model):
    for path, spec in REG["paths"].items():
        if model in spec["models"]:
            return path
    return None
def tier_of(model):
    for tier, models in REG["tiers"].items():
        if model in models:
            return tier
    return None
def resolve(tier, path):
    for model in REG["tiers"].get(tier, []):
        if find(model) == path:
            return model
    return None
def cmd_list(a):
    for path, spec in REG["paths"].items():
        print(f"{path}: {spec['label']}")
        for m in spec["models"]:
            print(f"  {tier_of(m) or '-':16} {m}")
def cmd_check(a):
    path = find(a.model)
    if path is None:
        print(f"REFUSED: {a.model!r} is not in any provider path", file=sys.stderr); return 2
    if a.path and path != a.path:
        print(f"REFUSED: {a.model!r} belongs to {path}, not {a.path}", file=sys.stderr); return 2
    tier = tier_of(a.model)
    if tier == "T3_ULTRA" and not a.approve_ultra:
        print(f"REFUSED: {a.model} is T3 Ultra; requires --approve-ultra", file=sys.stderr); return 3
    print(f"OK path={path} tier={tier} model={a.model}")
    return 0
def cmd_resolve(a):
    model = resolve(a.tier, a.path) if a.path else next((m for m in REG["tiers"].get(a.tier, []) if m in sum((p["models"] for p in REG["paths"].values()), [])), None)
    if model is None:
        print(f"no available {a.tier} model on path {a.path}", file=sys.stderr); return 2
    print(model); return 0
def cmd_gate(a):
    if a.tier != "T3_ULTRA":
        print("not gated"); return 0
    if not a.approve_ultra:
        print("REFUSED: T3 Ultra requires approval; ask principal (max 3/session)", file=sys.stderr); return 3
    print(f"APPROVED T3 Ultra consult: model={a.model or 'choose from T3_ULTRA'} reason={a.reason or 'unspecified'}"); return 0
p = argparse.ArgumentParser(prog="model-route")
sub = p.add_subparsers(dest="cmd", required=True)
l = sub.add_parser("list"); l.set_defaults(fn=cmd_list)
c = sub.add_parser("check"); c.add_argument("--path", choices=list(REG["paths"])); c.add_argument("--model", required=True); c.add_argument("--approve-ultra", action="store_true"); c.set_defaults(fn=cmd_check)
r = sub.add_parser("resolve"); r.add_argument("--tier", choices=list(REG["tiers"]), required=True); r.add_argument("--path", choices=list(REG["paths"])); r.set_defaults(fn=cmd_resolve)
g = sub.add_parser("gate"); g.add_argument("--tier", required=True); g.add_argument("--model"); g.add_argument("--reason"); g.add_argument("--approve-ultra", action="store_true"); g.set_defaults(fn=cmd_gate)
a = p.parse_args(); sys.exit(a.fn(a))
