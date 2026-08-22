# Workflow XML Model — RE_supermodel

Status: **CRM-14E + ENG-14 — XML RE_SUPERMODEL + definition validation**. The
XML definition format is the authoritative source for workflow definitions.
The workflow engine remains a generic, domain-neutral runtime. ENG-14 makes
invalid definitions fail deterministically at deployment via four explicit
validation layers (see §2b).

```
XML source
  ↓
Layer 1: generic XML parser     (workflow_app/xml/mini-xml.ts)        — well-formedness
Layer 2: engine grammar         (workflow_app/xml/xml-parser.ts)      — grammar
  ↓
ProcessGraph
  ↓
Layer 3: generic graph validator (workflow_app/xml/graph-validator.ts) — graph semantics
Layer 4: application contract    (workflow_app/definitions/application-contract.ts) — command routability
  ↓
versioned process_definitions row in Neon  (upsertProcessDefinition)
  ↓
workflow_engine execution
```

Future:

```
visual editor
  ↕  same XML source format
  ↓
parser → ProcessGraph → deploy → engine
```

---

## 1. Architectural decisions (re-stated)

1. **XML is the authoritative source format** for workflow definitions.
2. `ProcessGraph` remains the engine's internal/runtime representation.
3. XML parses/validates into `ProcessGraph`.
4. **The XML node id IS the workflow state identity.**
5. Human-readable `label` is presentation metadata for that same state.
6. No second CulebraLuxe workflow-state enum/mapping layer exists.
7. `deal.stage` remains a separate coarse canonical CRM state, changed only by
   explicit application commands.
8. SME/responsibility roles remain the same conceptual model already used by
   `workflow_app`; XML `responsibility` values are **hints** resolved by
   `workflow_app` to actual participants/users.
9. `workflow_engine` is completely domain-neutral and unchanged at runtime.
10. Simple cases take a short path; complex cases activate additional branches
    through the same model. Jurisdiction is never engine behavior.

---

## 2. Generic XML grammar

The grammar is deliberately small and generic. There are **no domain-specific
tags** (no `deal`, `offer`, `inspection`, `appraisal`, `CRIM`, `Florida`,
`PuertoRico`, `notario`). Domain concepts appear only as node `id`s, `label`s,
`responsibility` values, process variables, and conditions.

### Root

| Element | Attributes | Meaning |
|---|---|---|
| `process-definition` | `key` (req), `version` (req int>0), `name` (req), `description` (opt) | Deployment identity + metadata. |

### Node elements (exactly one `start-state`; all ids unique)

| Element | Attributes | Maps to `NodeDefinition` |
|---|---|---|
| `start-state` | `id`, `label`, `description` | `type: 'start'` |
| `state` | `id`, `label`, `description` | `type: 'state'` (passthrough) |
| `task-node` | `id`, `label`, `description`, `responsibility`, `priority`, `form-key` | `type: 'task'`; `responsibility` → `responsibility` + `candidateGroups: [responsibility]` |
| `command-node` | `id`, `label`, `description`, `command-type` (req), `transition`, `responsibility` | `type: 'command'`; `commandType`, `transition` (success transition) |
| `decision` | `id`, `label`, `description`, `refresh-facts` | `type: 'decision'`; child `<on/>` → `decisions` |
| `fork` | `id`, `label`, `description` | `type: 'fork'`; child `<transition required/>` → branches |
| `join` | `id`, `label`, `description` | `type: 'join'` |
| `timer` | `id`, `label`, `description`, `due-at`, `due-at-variable`, `on-fire` | `type: 'timer'`; `timer: { dueAt?, dueAtVariable?, transition? }` |
| `end-state` | `id`, `label`, `description`, `outcome` | `type: 'end'`; `outcome` ∈ `completed|cancelled|failed|conflict` |

### Child elements

| Element | Parent | Attributes | Maps to |
|---|---|---|---|
| `transition` | most node types | `name` (req), `to` (req), `condition` (opt), `required` (opt bool) | `TransitionDefinition` |
| `on` | `decision` | `condition` (req), `transition` (req, a transition *name*) | one entry of `decisions` |
| `display-order` / `node` | `process-definition` | `node ref` (req) | `ProcessGraph.displayOrder` (portal timeline) |

### Contract rules enforced by the parser (`xml-parser.ts`)

- duplicate node ids → rejected
- transition target that does not exist → rejected
- unknown element / attribute / non-empty text content → rejected explicitly
- more than one `start-state`, or none → rejected
- invalid `version` / `priority` / `outcome` / `refresh-facts` / `required` → rejected
- labels, descriptions, and responsibility metadata are preserved
- `display-order` refs must exist

### Contract rules enforced by the validator (`graph-validator.ts`)

The validator operates on **ProcessGraph**, not XML, so any future authoring
format (JSON, YAML, a visual editor) reuses it. It rejects only structures the
engine cannot run plus unambiguous authoring errors:

- exactly one start node; `startNodeId` points to it
- every transition target exists; no duplicate transition names on a node
- `command` nodes require `commandType` and a success transition
- `decision` nodes require transitions and every `<on>` rule must reference a
  declared transition name
- `timer` nodes require `due-at` or `due-at-variable` and a resume transition
- `end` nodes require a valid `outcome`
- cycles are allowed (blocker loops are intentional — the engine handles them)

The validator deliberately does **not** invent stylistic constraints.

---

## 2b. ENG-14 — Four validation layers / static analysis

Invalid workflow definitions fail deterministically at **deployment**, never
during live execution. The deploy pipeline
(`workflow_app/definitions/validate-definition.ts`, used by the generic deploy
command and by `parseReSupermodel()`) composes four explicit layers; each
reports actionable diagnostics:

| Layer | Module | Rejects |
|---|---|---|
| 1. XML well-formedness | `xml/mini-xml.ts` (`parseXml`) | non-well-formed input (unterminated/mismatched tags, unknown entities, processing instructions, DOCTYPE/DTD, trailing garbage) |
| 2. Engine grammar | `xml/xml-parser.ts` | unknown elements/attributes, duplicate node ids, missing/extra `start-state`, invalid attribute values, transition targets that do not exist |
| 3. Generic graph semantics | `xml/graph-validator.ts` | structures the engine cannot run (see below) — operates on `ProcessGraph`, so any future authoring format reuses it |
| 4. Application contract | `definitions/application-contract.ts` | `<command-node>` types with **no router case** in the canonical command inventory (`workflow_app/command-types.ts` / `lib/commands/register.ts`) |

Layer 3 additions (ENG-14):

- **Unreachable-node reporting** — any node with no path of transitions from
  the start node is an error (dead weight; usually a typo or leftover).
- **Unsupported-node diagnostics** — node types the engine has no handler for
  are rejected instead of silently degrading to passthrough behavior. The
  supported set is exactly the engine's dispatch surface: `start`, `end`,
  `task`, `decision`, `fork`, `join`, `timer`, `command`, and `state` (the
  explicit passthrough). `subprocess` gets a targeted diagnostic (declared in
  the engine type union but unimplemented).
- **Impossible-join / fork-join analysis, only where safely determinable** —
  a required fork branch that is a closed loop with no exit (no end/leaf/fork/
  join reachable) is an error: its token can never complete, so the process
  would hang and any join it feeds can never release. Required branches that
  never reach a join (or pass through a nested fork first) are **warnings**,
  not errors — the engine runs them, but the join-wait is trivially satisfied.
- **Cycles-allowed policy** — cycles are allowed and never reported as errors.
  Blocker loops (`work → issue → blocker → resolved → work`) are intentional
  and the engine handles them; only a cycle with **no exit** is rejected (it
  can never complete, which hangs the process).

Fail-fast contract: `deploy-process-definition.ts` (and every test consumer)
runs all four layers; any error aborts the deploy with `[layer]`-prefixed
diagnostics before the definition becomes runnable. `--dry-run` runs the same
validation without touching the database.

---

## 3. XML → ProcessGraph mapping (exact)

Given a node element `<E id="x" label="L" description="D" ...>`:

```
id                 -> NodeDefinition.id            (state identity)
label              -> NodeDefinition.name          (presentation label)
description        -> NodeDefinition.description
responsibility     -> NodeDefinition.responsibility
                     (task-node also -> candidateGroups:[responsibility])
command-type       -> NodeDefinition.commandType
transition (attr)  -> NodeDefinition.transition    (command success transition)
refresh-facts      -> NodeDefinition.refreshFacts
due-at             -> NodeDefinition.timer.dueAt
due-at-variable    -> NodeDefinition.timer.dueAtVariable
on-fire            -> NodeDefinition.timer.transition
outcome            -> NodeDefinition.outcome
<transition/>      -> NodeDefinition.transitions[]
<on/>              -> NodeDefinition.decisions[]
```

`ProcessGraph = { startNodeId, nodes, displayOrder? }`.

---

## 4. State identity + label contract (Story 116)

Every node may carry:

- `id` — stable machine/business workflow identity (the XML node id **is** the
  state)
- `label` — human-readable UI label
- `description` — optional explanatory copy

Example:

```xml
<task-node id="title_cure" label="Resolve Title Issues" responsibility="title_company"/>
```

The Portal read model exposes `node id`, `node label`, `node description`, and
`node responsibility` **directly from the deployed definition graph**. There is
no `PortalWorkflowState`, no `CulebraWorkflowState`, no translation table, and
no giant switch mapping for workflow node names.

`deal.stage` remains independent and changes only via explicit application
commands (`deal.set_stage_under_contract`, `deal.set_stage_closed`, ...).

---

## 5. Responsibility / SME contract (Story 117)

XML may declare one of the abstract business-role hints:

```
brokerage  buyer  seller  lender  inspector  appraiser
notario    title_company  other_sme
```

These are hints. `workflow_app/responsibility.ts` resolves them to an
operational owner class and, where relevant, a `deal_participant.role_label`
used to find the actual responsible SME. The engine never resolves application
identity — it only carries the hint (and mirrors it into `candidateGroups` on
task nodes for human-task candidates).

Important distinction preserved:

```
deal owner / accountable agent   !=   current task responsible SME
```

No second SME taxonomy was introduced.

---

## 6. RE_supermodel — text diagram

```
offer_accepted
      │
      ▼
pns_preparation ──cancel──► transaction_cancelled (cancelled)
      │ prepared
      ▼
pns_executed ──cancel──► transaction_cancelled
      │ executed
      ▼
mark_under_contract (command: deal.set_stage_under_contract)
      │
      ▼
under_contract
      │
      ▼
fork_tracks  (10 branches, all required; optional tracks gated by decisions)
  ├── title_work ──issue──► title_blocker ──resolved──► title_work
  ├── tax_clearance ──issue──► tax_blocker ──resolved──► tax_clearance
  ├── funds_ready ──issue──► funds_blocker ──resolved──► funds_ready
  ├── closing_documents ──issue──► closing_documents_blocker
  ├── inspection_applicable? ──► inspection ──issue──► inspection_blocker
  ├── financing_applicable? ──► financing ──fail──► financing_failed (failed)
  ├── appraisal_applicable? ──► appraisal ──issue──► appraisal_blocker
  ├── insurance_applicable? ──► insurance ──issue──► insurance_blocker
  ├── survey_applicable? ──► survey ──issue──► survey_blocker
  └── hoa_applicable? ──► hoa_clearance ──issue──► hoa_blocker
      (each '..._applicable?' decision routes "not applicable" straight to the join)
      │
      ▼
join_tracks
      │
      ▼
closing_readiness_gate (decision: closingConfirmationRequired == true)
      │ confirm (when required)                    │ ready (default)
      ▼                                            ▼
closing_readiness ──► ready_to_close        ready_to_close
(final brokerage confirmation)
                                                       │
                                                       ▼
                                                 closing_schedule (fork)
                                  ┌────────────────┴─────────────────┐
                                  │ closing (required)               │ deadline (optional)
                                  ▼                                  ▼
                            closing ──closed──►                closing_deadline_applicable?
                                  │ mark_closed                      │ monitor
                                  ▼                                  ▼
                            closed_state                        closing_date_timer
                                  │                              │ fire (date passed)
                                  ▼                              ▼
                            closing_schedule_join            closing_date_escalation
                                  │                       ┌─────┴──────┐
                                  │                       │ extend     │ proceed
                                  ▼                       ▼            │
                            post_closing              set_closing_date  │
                                  │                   (reschedule)      │
                                  ▼                       └──► timer ──┘
                            recording_applicable?
                                  │ run                    │ skip
                                  ▼                        ▼
                            recording ──issue──► recording_blocker ──► post_closing_complete
                                  │ done
                                  ▼
                            post_closing_complete (completed)
```

Terminals:

- `post_closing_complete` — completed
- `transaction_cancelled` — cancelled
- `transaction_failed` — failed
- `financing_failed` — failed

---

## 7. Simple cash path (Story 120)

A clean cash transaction flows through the **same** supermodel without any
placeholder work:

```
Offer Accepted → P&S Preparation → P&S Executed → Under Contract
→ Title/Legal → Tax/Municipal (CRIM) clearance → Funds Ready → Closing Documents
→ (inspection/financing/appraisal/insurance/survey/HOA all "not applicable" → join)
→ Closing Readiness (fact-gated) → Ready to Close → Closing → Closed
→ Recording/Registry follow-up (if requiresRegistryFollowup)
```

Because each optional track is gated by an applicability decision that routes
"not applicable" straight to the join, **no financing/appraisal/inspection/
HOA/survey/insurance task is ever created** for a simple case. This is proven
by scenario A in `workflow_app/tests/re-supermodel.test.ts`.

## 8. Complexity paths (Story 121)

Expressible without engine changes: inspection issue/repair, title defect/cure,
financing failure, appraisal gap, closing-date extension, tax/CRIM blocker,
HOA issue, survey issue, insurance blocker, funds-not-ready, and post-closing
recording/registry follow-up. Each is represented as a blocker loop
(`<track>` → issue → `<track>_blocker` → resolved → back) or a failure/terminal
edge.

## 9. Jurisdiction / configuration model (Story 119)

Operating differences are expressed **only as facts/capabilities** supplied by
`workflow_app`, never as jurisdiction-encoded engine behavior:

| Fact | Meaning |
|---|---|
| `closingAgentRole` | which closing professional (e.g. `notario` vs `title_company`) |
| `requiresNotario` | notary/closing professional required |
| `requiresTitleCompany` | title company work required |
| `requiresCrimClearance` | CRIM clearance required (PR/Culebra) |
| `requiresRegistryFollowup` | post-closing registry/recording follow-up |
| `requiresHoaClearance` | HOA/condo clearance required |
| `requiresSurvey` | survey required |
| `financingApplicable` | financing branch active (bool/null) |
| `appraisalApplicable` | appraisal branch active (bool/null); canonical deal.appraisal_required fact (CRM-19) |
| `lenderClearToClose` | lender clear-to-close (bool/null); canonical deal.lender_clear_to_close fact (CRM-20) |
| `closingDocumentsReady` | derived closing-document readiness (bool); packet catalog + transaction_document signed lineage (CRM-21) |
| `inspectionApplicable` | inspection branch active (bool/null) |
| `insuranceApplicable` | insurance branch active (bool/null) |
| `closingConfirmationRequired` | optional final brokerage confirmation |
| `closingDateScheduled` / `closingDate` | closing-date monitor fact/date |

CulebraLuxe currently supplies the PR/Culebra facts/config. Florida material is
a complexity reference/stress model, not the current production jurisdiction.

## 10. Appraisal independence (Story 123)

`appraisalApplicable` is independent of `financingApplicable`:

- cash + `appraisalApplicable` ⇒ appraisal runs, financing does not (scenario C)
- financed + `appraisalApplicable=false` ⇒ financing runs, appraisal does not
  (scenario D)

Neither "financed ⇒ appraisal" nor "cash ⇒ no appraisal" is encoded.

## 10b. Appraisal applicability resolution (Story CRM-19)

`appraisalApplicable` is a canonical deal-level fact, never derived from
provider-specific logic:

- durable source: `deal.appraisal_required` (bool, null = unresolved)
  — migration `db/migrations/031_deal_appraisal_required.sql`
- explicit resolution: the application-only command `deal.set_appraisal_required`
  (`db/deal-appraisal.ts`), routed but never referenced by a workflow
  command-node, mirroring `deal.set_financing_type`
- the workflow decision reads the fact from the application projection
  (`workflow_app/facts.ts` → `appraisalApplicableFromRequired`); the generic
  engine only evaluates the decision

`null`/unknown is handled explicitly, never silently skipped: the
`appraisal_applicable` decision routes `appraisalApplicable == null` to the
`appraisal_applicability_unresolved` task ("Resolve Appraisal Applicability",
brokerage). `resolved` re-evaluates the decision with refreshed facts (a
blocker loop until a human/application resolves the fact); `escalate`
terminates the transaction as failed.

## 10c. Lender clear-to-close (Story CRM-20)

`lenderClearToClose` is a canonical deal-level fact, never provider behavior:

- durable source: `deal.lender_clear_to_close` (bool, null = unresolved /
  not-applicable) — migration `db/migrations/032_deal_lender_clear_to_close.sql`
- explicit resolution: the application-only command
  `deal.set_lender_clear_to_close` (`db/deal-lender-clearance.ts`), routed but
  never referenced by a workflow command-node, mirroring
  `deal.set_financing_type` / `deal.set_appraisal_required`
- the workflow decision reads the fact from the application projection
  (`workflow_app/facts.ts` → `lenderClearToCloseFromFact`); the generic engine
  only evaluates the decision

Consumed **only for financed deals** (`financingApplicable == true`): the
closing-readiness gate routes financed deals through `lender_clearance_gate`,
where `lenderClearToClose == true` proceeds, `null` surfaces the explicit
`lender_clearance_resolution` task ("Resolve Lender Clear-to-Close",
brokerage), and `false` lands in `lender_clearance_pending` ("Lender Clearance
Pending", lender) — the deal can never appear closing-ready before lender
clearance. Cash/non-financed deals are routed around the fact entirely, so they
are unaffected.

## 10d. Closing-document readiness (Story CRM-21)

`closingDocumentsReady` is a **derived** fact, never a persisted boolean and
never a bare human checkmark:

- required set: the packet catalog's closing package
  (`workflow_app/transaction-packet.ts` → `requiredClosingDocumentPackage`:
  the required items at the closing stage whose `documentType` is `closing` —
  deed / closing package and closing statement). Post-closing items (registry /
  recording follow-up) are excluded: closing-document readiness is assessed
  before the closing. The fact never invents a required document.
- signed/final lineage: an item is ready only when a matching
  `transaction_document` row is in the final `signed` state of the DOC-01
  `draft -> ready -> sent -> signed` lineage (DOC-05 reconciliation appends a
  NEW signed media row and transitions to `signed`). Pre-signed states
  (`draft`/`ready`/`sent`) are not ready; `voided`/`superseded` rows never
  count, so a superseded signed artifact loses readiness while its replacement
  is still a draft.
- deterministic: duplicate/replayed rows cannot change the result.
- consumption: `closing_documents_gate` (a decision, `refresh-facts="true"`)
  sits between the fork/join and `closing_readiness_gate`. It routes
  `closingDocumentsReady == true` to readiness; `false` lands in
  `closing_documents_pending` ("Closing Documents Pending", brokerage) and
  blocks readiness — `resolved` re-evaluates the gate with refreshed facts (a
  blocker loop until the packet is complete and signed), `escalate` terminates
  the transaction as failed. The gate runs before the lender gate, so a
  financed deal cannot reach lender clearance while its closing documents are
  not ready.

## 11. Closing readiness (Story 124 / 136)

Eligibility is structural, never a magic boolean. The fork/join already
guarantees every applicable required obligation is cleared/waived/resolved
before the readiness segment is reached. The `closing_documents_gate` first
requires the canonical closing-document packet to be complete AND signed/final
(`closingDocumentsReady == true`, CRM-21, §10d); `false` lands in
`closing_documents_pending` and blocks readiness. Financed deals
(`financingApplicable == true`) are then routed through `lender_clearance_gate`
(CRM-20, §10c): lender clear-to-close must be `true` before readiness — `null`
surfaces an explicit resolution task, `false` is a pending state that blocks
readiness. Cash/non-financed deals skip the lender fact entirely. The gate then
only adds the final brokerage confirmation when the `closingConfirmationRequired`
fact is set; otherwise it proceeds straight to `ready_to_close`. The
confirmation cannot override blockers because it comes AFTER the join. (Story
135/136 improved this: the `closingReadinessVerified` boolean and its command
were removed as the wrong semantic shape — confirmation is a task, not a
persisted fact.)

The command inventory for `RE_supermodel-v1.xml` is complete and guarded
(CRM-14G): the only command-nodes are `mark_under_contract`
(`deal.set_stage_under_contract`), `mark_closed` (`deal.set_stage_closed`), and
`set_closing_date` (`deal.set_closing_date`) — each has a router case in
`workflow_app/command-router.ts`. `deal.set_financing_type`,
`deal.set_appraisal_required` (CRM-19), and `deal.set_lender_clear_to_close`
(CRM-20) are routed but application-only (their facts are read by the workflow,
never set via a workflow command), and there is no
`deal.set_closing_readiness_verified` command. The authoritative registry is
`workflow_app/command-types.ts`; a command-node added to the XML without a
router case fails `workflow_app/tests/command-inventory.test.ts` and
`parseReSupermodel()`.

## 12. P&S / closing date semantics (Story 122)

`closing_date_timer` uses `dueAtVariable="closingDate"` (the canonical deal
closing date). If the date is amended/extended, `closing_date_escalation` →
`deal.set_closing_date` → the timer reschedules on the **same** instance; the
workflow never restarts because a date changed (scenario I). `deal.stage`
closing is a separate command.

## 13. Post-closing (Story 125)

`mark_closed` fires `deal.set_stage_closed` (deal.stage becomes closed) while
the workflow continues through `post_closing` → `recording_applicable` →
`recording` (registry/recording follow-up) → `post_closing_complete`. Workflow
completion is not collapsed into `deal.stage`.

---

## 14. Generic deployment pipeline (Story 127)

Canonical command (created but **not executed** in CRM-14E — nothing is
deployed to Neon):

```sh
node_modules/.bin/tsx workflow_app/scripts/deploy-process-definition.ts \
  workflow_app/definitions/RE_supermodel-v1.xml
```

Pipeline: `XML → parse → validate → ProcessGraph → upsertProcessDefinition`.
Versioned and idempotent; the same command deploys any future definition.

## 15. Visual modeler future contract (Story 129)

Documentation only — **no visual editor is built by CRM-14E.**

```
visual editor
  ↕  (read/write the SAME XML source)
  ↓
parser (mini-xml → xml-parser)
  ↓
validator (graph-validator)
  ↓
deploy (upsertProcessDefinition)
  ↓
engine
```

The editor and the pipeline share one XML grammar, so the XML remains
source-controlled, diffable, reviewable, and versioned. A future editor only
needs to emit/read the grammar in section 2; no engine change is required.

---

## 16. Dependency / package requirement

**No new package is required.** The repository had no XML parser dependency;
Node has no built-in XML parser; a bounded, deterministic parser
(`workflow_app/xml/mini-xml.ts`) was implemented for the controlled grammar.
If full XML 1.0 conformance (DTDs, namespaces, arbitrary entity expansion) is
ever needed, replace `mini-xml.ts` behind the same `parseXml` surface with a
maintained parser such as `fast-xml-parser` or `sax`.
