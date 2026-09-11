# knip intent register

Why this file exists: knip answers **"is anything importing this?"** — a
*reachability* question. Reachability is not liveness. "No importers" has three
different meanings and knip cannot tell them apart:

1. **dead** — refactored past it. Safe to remove.
2. **wet cement** — not wired yet; the wiring is the next step, not a fix.
3. **platform capability** — a tool that exists so it can be used later (the MQ
   and workflow surfaces). Having no importers today is its *correct* state.

Only category 1 is a defect. So intent is declared here, once, by a human, and
`knip.json` restates these globs in its `ignore`. Nothing is hidden that wasn't
explicitly claimed: **each entry below must state why it is alive.**

## A. Platform / wet cement — the captain's, deliberate

Report, never remove. The captain's words: *"DO NOT TOUCH WORKFLOW, it's too
complicated, it doesn't know how it works, it's the wrong tool for this"* and
*"mq is very important, those are mine."*

| Path | Why it is alive |
| --- | --- |
| `workflow_engine/**` | A whole working surface the captain uses. |
| `grok/**` | The captain's; vendored flight-recorder source. |
| `lib/workflow/**` | Platform contracts, wired when the engine needs them. |
| `lib/mq/**` | Message queue. **Explicitly important.** |
| `workflow_app/scripts/**` | Operator scripts, run by hand. |
| `workflow_app/idempotency.ts` | Platform capability. |
| `components/portal/workflow-dashboard-card.tsx` | Wet cement for the workflow surface. |

## B. Reached without an import — real code knip cannot see

Verified individually; do **not** delete any of these.

| Path | Why knip cannot see it |
| --- | --- |
| `neon.ts` | Imported by `agent-runtime/capabilities.ts` and others (5 importers) — reachable only from library modules that are themselves reached from scripts/tests. |
| `app/actions/catchup-lead.ts` | A server action invoked by `scripts/catchup-dev-proof.ts` and asserted by `workflow_app/tests/catch-up.test.ts`. |
| `lib/intake/index.ts` | `workflow_app/tests/intake-contract.test.ts` reads it **by path** (`readFileSync`). No import graph can reveal this. This one already broke a test once. |

## C. Dependency findings that are wrong

| Dependency | Why it is used |
| --- | --- |
| `tailwindcss` | Consumed through `@tailwindcss/postcss` in `postcss.config.mjs`; knip does not follow the PostCSS plugin for Tailwind v4. |
| `@fullcalendar/core` | Required peer of `@fullcalendar/react`, which is used. |
| `shadcn` | Tooling, invoked by CLI rather than imported. |
| `tw-animate-css` | CSS shipped for Tailwind; imported from CSS, not from TS. |

## What knip is still allowed to drive

Category-1 removals only, and only after **two** checks: (1) no import specifier
anywhere, **and** (2) the path string appears nowhere (tests and scripts read
files by name). Always re-run the check after a deletion wave — dead files hide
behind each other, since a dead file is often the *only* importer of another.
