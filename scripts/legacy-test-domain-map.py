#!/usr/bin/env python3
"""Categorise the legacy TypeScript tests by the part of the product they test.

The question this answers is "what is this file about", not "is it covered": a test of the
forms template registry, a test of the MQ broker and a test of the Forge lead routing are
three different piles of work even when all three import the same fake-SQL harness.

How the subject is picked, and why:

  - The **rarest import** is the subject. `db/query-executor` is imported by 75 legacy tests
    because it is the fake transaction runner every DB test wires up; a module that four files
    import is far more likely to be the thing the file is actually about.
  - Modules on the PLUMBING list are never evidence of a subject, however rare, because they
    are the harness rather than the behaviour under test.
  - The subject maps to an area through AREA_RULES, first match wins, and the area path is the
    product's own shape: `ui/<tier>/<subject>` for the browser tier, `core/<subject>` for the
    Rust domain and database crates, and one segment where the area is a whole subsystem
    (forge, workflow-engine, mq, services, agent-runtime).

Usage:
  python3 scripts/legacy-test-domain-map.py            # summary only
  python3 scripts/legacy-test-domain-map.py --write    # + docs/agent/legacy-test-parity/domains.tsv
"""
from __future__ import annotations

import argparse
import collections
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEGACY = os.path.join(ROOT, "legacy")
TSV = os.path.join(ROOT, "docs/agent/legacy-test-parity/domains.tsv")
DOC = os.path.join(ROOT, "docs/agent/LEGACY-TEST-DOMAINS.md")
STATUS_TSV = os.path.join(ROOT, "docs/agent/legacy-test-parity/status.tsv")
COVERED_TSV = os.path.join(ROOT, "docs/agent/legacy-test-parity/already-covered.tsv")

IMPORT_RE = re.compile(r"from\s+'([^']+)'")
CASE_RE = re.compile(r"^\s*(?:it|test)\s*[.(]\s*['\"`](.+?)['\"`]", re.M)

# The harness, not the subject. Seen across dozens of files each; a file "about" one of these
# is a file about something else that happens to use it. A prefix entry covers a directory.
PLUMBING_NAMES = {
    "db/query-executor",
    "db/tx",
    "db/query",
    "lib/neon-interactive",
    "lib/portal-write-error",
    "test-support",
    "test-doubles",
    "types",
    "fixtures",
    "fake-sql",
    "harness",
    "schema-parity",
    "service-error-sink",
    "runtime-inspector",
}
PLUMBING_PREFIXES = ("scripts/", "test-support/", "testv2/engine_tests/fixtures")


# area -> pattern; first match wins on the subject module, then on the file name.
AREA_RULES: list[tuple[str, str]] = [
    # --- whole subsystems -------------------------------------------------------------
    (r"^lib/mq\b|outbox|message-broker|proof-consumer|crm26-consumer", "mq"),
    (r"^workflow_engine/|^lib/workflow\b|engine_tests|workflow-engine|workflow-spec|"
     r"definition|split-join|split-child|join-ids|expressions|causal-graph|"
     r"^workflow_app/(xml|start-core|reset|query|reconcile|deadline|responsibility|"
     r"uniqueness|transaction-packet)|mini-xml|graph-validator|dynamic-fork|"
     r"execution-target|^lib/compare", "workflow-engine"),
    (r"^workflow_app/(forge|definitions/forge|forge-)|forge-|-forge-|^forge/|smith-|scout-|"
     r"lead-|lead-routing|qa-|release-operations|storyboard-execution|sdlc|factory|"
     r"role-mapping|architect|inspector", "forge"),
    (r"^agent-runtime/|harness|assay|candidate|opencode|openclaw|orchestrat|invoker|lane-|-lane|"
     r"readiness|recovery|write-policy|run-guardrails|git-packet|repo-context|loop|team|"
     r"skills|deepseek|gateway|dispatch|executor|agent-|run-machine|silent-failure|"
     r"slack-notifier", "agent-runtime"),
    # --- repo, CI and the worker workspace (the engine's own plumbing) ----------------
    (r"^lib/git|branch-hygiene|sync-conflict|worker-workspace|^scripts/|trailing-whitespace|"
     r"process-profiler|db-boundary|db-routing|rules-have-guards|dependency-triage|"
     r"release-ci|use-server-exports|neon-interactive|schema-parity|column-writer", "platform"),
    # --- the service tier -------------------------------------------------------------
    (r"^services/|legacy/services|service-registry|service-error|composition|"
     r"authorization|entitlement|ownership|service-construction|base-service", "services"),
    # --- browser tier -----------------------------------------------------------------
    (r"ui/client-workspace|client-workspace-controller|client-workspace-channel|"
     r"channel-projection", "ui/core/clients"),
    (r"^lib/navigation|^lib/portal|portal-write|portal/types|portal-|^lib/favorites|"
     r"^lib/saved-searches", "ui/core/portal"),
    (r"^lib/storyboard-data|storyboard-yew|yew|sorter-board|sorter|storyboard",
     "ui/core/storyboard"),
    (r"ui-lab|framer|islands|^lib/ara|ara-|command-status-band|design-system",
     "ui/core/design-system"),
    (r"ui/|forms-editor|catch-up-editor|editor", "ui/core/forms"),
    # --- the flight recorder (the run trace, read back) --------------------------------
    (r"flight-recorder|flight_recorder", "core/flight-recorder"),
    # --- form documents ---------------------------------------------------------------
    (r"^lib/forms|forms/|-forms|forms-|template-registry|signature-font|forms-font|"
     r"forms-geometry|forms-format|doc08|pdf", "core/forms"),
    # --- clients and relationships ----------------------------------------------------
    (r"^db/client|client-admin|client-room|relationship-intel|promote-evidence|"
     r"contact-history|relationship-evidence|consolidation|apple-contact", "core/clients"),
    # --- the rest of the domain -------------------------------------------------------
    (r"accounting|receivable|expense|ledger|invoice|bank|ofx", "core/accounting"),
    (r"agreement|signature|signing|sign-|contract-service|contract-facts|"
     r"(^|/)contract", "core/signature"),
    (r"whatsapp|comms|message|mail|gmail|imap|inbox|conversation|burst", "core/comms"),
    (r"calendar|reminder|closing-timer|closing-date|showing|offer|catch-up|catchup|"
     r"eventkit", "core/calendar"),
    (r"property|listing|regrid|syndication|marketing|website|landing|public-listing|"
     r"media|gallery|publication|publishing|appraisal", "core/property"),
    (r"project|wbs|task|transaction-document|document", "core/projects"),
    (r"deal|pipeline|closing|commission|financing|lender|decision-analysis|"
     r"seller-strategy", "core/deals"),
    (r"security|auth|session|require-portal-access|sign-in|guest|vault|security-audit|"
     r"break-glass", "core/security"),
    (r"person|firm|organisation|organization|team-member|contact|address-format|"
     r"normalize-phone|phone-identit", "core/people"),
    (r"issue|support|guide|tech|anomaly|diagnostic|factura|kpi", "core/operations"),
    # --- the database and the engine's persistence ------------------------------------
    (r"^db/|migration|schema|pg_|sql_|pool|retry|receipt|unit-of-work|broker-signature|"
     r"command-receipt|agent-work|workflow_ops", "core/db"),
    (r"^lib/commands|command-layer|command-types|dispatcher", "core/commands"),
]

# What each area is, and where Rust serves it today. The Rust path is what a reviewer can open;
# it is not a claim that the behaviour is pinned.
AREA_NOTES: dict[str, tuple[str, str]] = {
    "forge": ("the SDLC engine: roles, routing, findings, QA verdicts, repair ledger, release",
              "rust/forge/src/{roles,routing,engine,evidence,release}"),
    "agent-runtime": ("the lane harness: role runner, provider gateway, worker workspace, dispatch",
                      "rust/forge/src/{runtime,execution}, rust/forge/src/engine"),
    "workflow-engine": ("the workflow engine proper: definitions, expressions, store, split/join",
                        "rust/core/workflow/src, rust/forge/src/engine"),
    "mq": ("the message queue: outbox, subscriptions, leases, retries, proof consumers",
           "rust/core/db/src/outbox.rs, rust/server/tests/mq_runtime_dev.rs"),
    "services": ("the service tier: BaseService, authorization, registry, projections",
                 "rust/core/service/src"),
    "platform": ("repo and CI guards, worker workspace, the engine's own tooling",
                 "rust/cli/src/forge/{repo_guards,lint}, rust/forge/src/sync_conflict.rs, .githooks/"),
    "core/forms": ("form documents: template registry, versioning, geometry, signatures, PDF",
                   "rust/core/domain/src/{forms,forms_template,forms_geometry,forms_execution}"),
    "core/clients": ("clients and relationships: identity, evidence, promotion, client room",
                     "rust/core/domain/src/{client,client_room,relationship_evidence}, rust/core/db/src/client"),
    "core/db": ("schema, migrations, query discipline, receipts, unit of work",
                "rust/core/db/src, db/migrations"),
    "core/security": ("auth, sessions, guest access, vault, security audit",
                      "rust/core/auth/src, rust/core/domain/src/security.rs, rust/core/db/src/security.rs"),
    "core/calendar": ("calendar, catch-up, reminders, closings, showings, offers",
                      "rust/core/domain/src/{calendar,catch_up,showing}.rs"),
    "core/property": ("listings, media, syndication, marketing, public listing",
                      "rust/core/domain/src/{property,media,marketing,public_listing}.rs"),
    "core/projects": ("projects, work breakdown, tasks, transaction documents",
                      "rust/core/domain/src/{project,wbs,task}.rs"),
    "core/deals": ("deals, financing, lender clearance, seller strategy",
                   "rust/core/db/src/deal_portal, rust/core/domain/src/contract.rs"),
    "core/signature": ("agreements, signatures, signing execution",
                       "rust/core/domain/src/{contract,signature,forms_broker_signature}.rs"),
    "core/comms": ("messages: Gmail, Apple Mail, WhatsApp, conversation bursts",
                   "rust/core/domain/src/{gmail,applemail,apple_messages,comms}.rs"),
    "core/people": ("people, firms, addresses, contact identity",
                    "rust/core/domain/src/{person,firm}.rs"),
    "core/accounting": ("receivables, expenses, invoices, bank feeds",
                        "rust/core/domain/src/accounting.rs, rust/core/db/src/accounting"),
    "core/commands": ("the command bus: contracts, dispatcher, receipts",
                      "rust/core/service/src/command.rs, rust/core/db/src/command_receipt.rs"),
    "core/operations": ("issues, support, guides, diagnostics, accounting",
                        "rust/core/domain/src/{issue,support,guide,tech,accounting}.rs"),
    "core/flight-recorder": ("the run trace: records, views, id guards",
                             "rust/core/domain/src/flight_recorder.rs, rust/core/db/src/flight_recorder"),
    "ui/core/portal": ("portal shell: navigation, write errors, shared read models",
                       "rust/ui/src/app/{shell,chrome}.rs"),
    "ui/core/forms": ("the forms screens", "rust/ui/src/app/screens/forms"),
    "ui/core/clients": ("the client workspace screen", "rust/ui/src/app/screens/clients.rs"),
    "ui/core/storyboard": ("the Story Board screens and read models",
                           "rust/ui/src/app/screens/storyboard.rs, rust/core/db/src/forge_read.rs"),
    "ui/core/design-system": ("shared components and the UI lab",
                              "rust/ui/src/app/screens/ui_lab.rs, rust/ui/src/app/template.rs"),
}




def norm(target: str) -> str:
    """`@/legacy/db/client/x` -> `db/client/x`; a relative import stays as written."""
    t = re.sub(r"^@/legacy/", "", target)
    t = re.sub(r"^@/", "", t)
    t = re.sub(r"^\.+/", "", t)
    return t.replace("\\", "/")


def imports_of(src: str) -> list[str]:
    out = []
    for t in IMPORT_RE.findall(src):
        if t.startswith(("node:", "vitest")) or not t.startswith((".", "@")):
            continue
        n = norm(t)
        if n:
            out.append(n)
    return out


def area_for(subject: str, rel: str) -> str:
    """The subject module decides; the file's own path is the fallback."""
    for pattern, area in AREA_RULES:
        if re.search(pattern, subject):
            return area
    for pattern, area in AREA_RULES:
        if re.search(pattern, rel):
            return area
    return "unclassified"


def is_plumbing(module: str) -> bool:
    return module in PLUMBING_NAMES or module.startswith(PLUMBING_PREFIXES)


def classify(mods: list[str], counts: collections.Counter, rel: str) -> tuple[str, str, int]:
    """Vote with every import the file makes: each one votes for its area, a rare import
    voting louder than a common one. A file that pulls three forge modules and one client
    module is a forge file even when the client module is the rarer of the four.

    Returns (area, evidence module, its import count).
    """
    weight: dict[str, float] = {}
    best_of: dict[str, tuple[int, str]] = {}
    for m in mods:
        if is_plumbing(m):
            continue
        area = area_for(m, "")
        if area == "unclassified":
            continue
        weight[area] = weight.get(area, 0.0) + 1.0 / counts[m]
        if area not in best_of or counts[m] < best_of[area][0]:
            best_of[area] = (counts[m], m)
    if weight:
        # AREA_RULES order breaks ties, so the result does not depend on dict ordering.
        order = []
        for _pattern, area in AREA_RULES:
            if area in weight and area not in order:
                order.append(area)
        winner = max(order, key=lambda a: weight[a])
        # A weak import vote (one module, or only common ones) loses to the file's own name:
        # `forge-story-batch-deploy.test.ts` imports `sql` from db/client as a runner, and the
        # name is the better statement of what it is about.
        if weight[winner] < 0.2:
            by_name = area_for("", rel)
            if by_name != "unclassified" and by_name != winner:
                return by_name, "(path)", 0
        return winner, best_of[winner][1], best_of[winner][0]
    area = area_for("", rel)
    return area, "(path)", 0


def effective_status() -> dict[str, str]:
    """file -> status. The hand-written row wins over the machine scan, which only ever fills
    an `unassessed` row (same precedence as `scripts/legacy-test-parity.sh`)."""
    out: dict[str, str] = {}
    if os.path.exists(STATUS_TSV):
        for line in open(STATUS_TSV, encoding="utf-8"):
            if line.startswith("#") or not line.strip():
                continue
            parts = line.rstrip("\n").split("\t")
            if parts[0]:
                out[parts[0]] = parts[1] if len(parts) > 1 else ""
    if os.path.exists(COVERED_TSV):
        for line in open(COVERED_TSV, encoding="utf-8"):
            if line.startswith("#") or not line.strip():
                continue
            parts = line.rstrip("\n").split("\t")
            if len(parts) >= 3 and out.get(parts[0]) == "unassessed":
                out[parts[0]] = parts[2]
    return out


def write_doc(rows: list[tuple], by_area: collections.Counter,
              cases_by_area: collections.Counter) -> None:
    status = effective_status()
    order = ["ported", "in_force_unported", "missing_capability", "diverged", "retired",
             "held_back", "already_covered", "gap", "unassessed"]
    per_area: dict[str, collections.Counter] = {a: collections.Counter() for a in by_area}
    for rel, area, _subj, _w, _cases in rows:
        per_area[area][status.get(rel, "not_in_ledger")] += 1

    lines = []
    add = lines.append
    add("# Legacy tests by domain — what the 465 files are about")
    add("")
    add("Generated by `python3 scripts/legacy-test-domain-map.py --write` from the legacy files")
    add("themselves. `docs/agent/legacy-test-parity/domains.tsv` is the per-file table; this page is")
    add("the same data as a map. The conversion ledger (`docs/agent/LEGACY-TEST-PARITY.md`) answers")
    add("*how far along* each file is; this answers *what pile it belongs to*, which is the question")
    add("to sort by when the queue is worked area by area.")
    add("")
    add("## How a file gets its area")
    add("")
    add("1. Every import votes for the area its module is part of, a rarely-imported module voting")
    add("   louder (`1 / files that import it`). `db/query-executor` is the fake transaction runner in")
    add("   75 files; it votes, but it cannot decide a file on its own.")
    add("2. The harness never votes: `db/query-executor`, `db/tx`, `lib/portal-write-error`, `fake-sql`,")
    add("   `harness`, `test-support/`, `scripts/`, fixtures. These are how a test is wired, not what it")
    add("   is about.")
    add("3. A weak vote (under 0.2, i.e. one common module) loses to the file's own name, because")
    add("   `forge-story-batch-deploy.test.ts` imports `sql` from `db/client` simply to run SQL.")
    add("4. A file with no imports at all is classified from its path.")
    add("")
    add("An area is a *location in the product*, not a claim about coverage. A file in `core/forms`")
    add("is a file about forms; nothing here says whether Rust pins it yet — that is column one of the")
    add("ledger, and the two are joined in the queue table below.")
    add("")
    add("## The map")
    add("")
    add("| area | files | legacy cases | what it is | Rust serves it in |")
    add("| --- | --- | --- | --- | --- |")
    for area in sorted(by_area, key=lambda a: (-by_area[a], a)):
        what, home = AREA_NOTES.get(area, ("-", "-"))
        add(f"| `{area}` | {by_area[area]} | {cases_by_area[area]} | {what} | `{home}` |")
    add(f"| **total** | **{len(rows)}** | **{sum(cases_by_area.values())}** | | |")
    add("")
    add("## The queue by area")
    add("")
    add("| area | files | unassessed | gap | already_covered | ported | other |")
    add("| --- | --- | --- | --- | --- | --- | --- |")
    for area in sorted(by_area, key=lambda a: (-per_area[a]["unassessed"], a)):
        c = per_area[area]
        other = sum(v for k, v in c.items()
                    if k not in ("unassessed", "gap", "already_covered", "ported"))
        add(f"| `{area}` | {by_area[area]} | {c['unassessed']} | {c['gap']} | "
            f"{c['already_covered']} | {c['ported']} | {other} |")
    add("")
    add("## Regenerating and re-labelling")
    add("")
    add("```sh")
    add("python3 scripts/legacy-test-domain-map.py            # the counts, on stdout")
    add("python3 scripts/legacy-test-domain-map.py --write    # + domains.tsv and this page")
    add("```")
    add("")
    add("The script is the single writer of both artifacts: a file's area is corrected by editing")
    add("`AREA_RULES` in the script (one rule per area, first match wins), never by editing the TSV —")
    add("a hand-edited row would be overwritten on the next run and would give one fact two authors.")
    add("")
    with open(DOC, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))
    print(f"wrote {os.path.relpath(DOC, ROOT)}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--write", action="store_true")
    args = ap.parse_args()

    files = []
    for dirpath, _dirs, names in os.walk(LEGACY):
        for name in names:
            if name.endswith(".test.ts"):
                files.append(os.path.join(dirpath, name))
    files.sort()

    # how widely each module is imported: the rarity weight
    counts: collections.Counter = collections.Counter()
    parsed = []
    for path in files:
        src = open(path, encoding="utf-8", errors="replace").read()
        mods = list(dict.fromkeys(norm(t) for t in imports_of(src)))
        parsed.append((path, src, mods))
        for m in mods:
            counts[m] += 1

    rows = []
    by_area: collections.Counter = collections.Counter()
    cases_by_area: collections.Counter = collections.Counter()
    for path, src, mods in parsed:
        rel = os.path.relpath(path, ROOT)
        area, subject, weight = classify(mods, counts, rel)
        cases = len(CASE_RE.findall(src))
        by_area[area] += 1
        cases_by_area[area] += cases
        rows.append((rel, area, subject, weight, cases))

    rows.sort(key=lambda r: (r[1], r[0]))
    areas = sorted(by_area)
    peak = max(by_area.values())

    print(f"{len(rows)} legacy test files in {len(areas)} areas")
    width = max(len(a) for a in areas)
    for a in areas:
        bar = "#" * max(1, round(by_area[a] / peak * 40))
        print(f"  {a:<{width}}  {by_area[a]:4d} files  {cases_by_area[a]:5d} cases  {bar}")

    unclassified = [r for r in rows if r[1] == "unclassified"]
    if unclassified:
        print(f"\nunclassified ({len(unclassified)}) - subject module in the note:")
        for rel, _a, subject, weight, _c in unclassified[:20]:
            print(f"  {rel}  [{subject} x{weight}]")
        if len(unclassified) > 20:
            print(f"  ... and {len(unclassified) - 20} more")

    if args.write:
        os.makedirs(os.path.dirname(TSV), exist_ok=True)
        with open(TSV, "w", encoding="utf-8") as fh:
            fh.write("file\tarea\tsubject\tsubject_imports\tcases\n")
            for rel, area, subject, weight, cases in rows:
                fh.write(f"{rel}\t{area}\t{subject}\t{weight}\t{cases}\n")
        print(f"\nwrote {os.path.relpath(TSV, ROOT)}")
        write_doc(rows, by_area, cases_by_area)


if __name__ == "__main__":
    main()
