# The Real-Estate Transaction — domain design

Status: **design captured 2026-09-11.** No workflow build tonight (deliberate).
This document exists so the shape survives the session that discovered it.

---

## 1. The one-paragraph model

A real-estate transaction is **one process** whose **artifacts are contracts**, whose
**evidence is documents**, whose **inputs are forms**, and whose **human work is tasks**.
Everything else (person, property, project) is an identity or a place to hang the work.

```
        Forms ──────► Contract ──────► Vault
        (input)       (artifact)       (evidence)
            │             │                │
            └────────► PROCESS ◄───────────┘        ← the transaction itself
                    (state machine)
                          │
                     Projects / WBS
                      (human work)
```

Everything above already exists in this repo. What is missing is **one cord** (section 5).

---

## 2. What already exists (verified, not aspirational)

### The engine — generic, domain-free, running
`process_definitions` · `process_instances` · `process_events` (partitioned, 858 rows in PROD)
· `process_commands` · `workflow_command_receipt` · `workflow_execution_trace_event`
· `workflow_task_correlation`

```
process_instances:  business_key · subject_type · subject_id · parent_instance_id
                    root_token_id · variables · status · outcome
```

`subject_type`/`subject_id` are **polymorphic** — an instance is *about* something.
`parent_instance_id` gives **nested instances**. 7 instances are live in PROD.

### The supermodel — `workflow_app/definitions/RE_supermodel-v1.xml`
The authoritative residential transaction. XML -> ProcessGraph -> validated -> deployed.
"It is jurisdiction-neutral; differences are facts, never node names."

```
1 start-state   16 node/decision   32 task-node   4 end-state
16 decision      5 command-node     3 timer       2 fork   2 join
138 transitions
```

States, as deployed: `pns_preparation` -> `pns_executed` -> `mark_under_contract` ->
`title_work` · `tax_clearance` · `funds_ready` · `closing_documents` · `inspection` ·
`financing` · `appraisal` · `insurance` · `survey` · `hoa_clearance` ·
`lender_clearance_*` -> `closing_readiness` -> `closing` -> `mark_closed` -> `recording`.

Live proof it runs on the **business** side, not just Forge:

```
definition 0c5df2c3  subject story/ENG-FORG...  x4   <- Forge SDLC
definition f51d9112  subject deal/eb7cbd90      x1   completed
definition f51d9112  subject deal/60000000      x1   active
definition f51d9112  subject deal/60000000      x1   error/failed   <- loose end, 5.4
```

### The Forge pain, paying off on the business side

| what cost us pain in Forge | what the business side already has |
|---|---|
| splits / forks | 2 fork + 2 join — due diligence runs in parallel (title, tax, funds, docs, inspection, financing, appraisal, insurance, survey, HOA) joining at `closing_readiness` |
| holds | the `*_blocker` states (`title_blocker`, `tax_blocker`, `appraisal_blocker`, `inspection_blocker`, ...) |
| human waits | **32 task-node**s — the engine writes real task rows (`tasks` table with `node_id` + `process_instance_id`), which is exactly "an offer isn't valid until the seller acts" |
| timers / escalation | `inspection_deadline_escalation`, `financing_deadline_escalation`, `closing_date_escalation` (driven by `dueAtVariable`) |
| decisions / routing | 16 decisions — applicability of appraisal, financing, inspection, insurance, survey, HOA — with **NULL never silently skipped**: unresolved applicability routes to an explicit human task |
| command nodes | 5 automated steps |
| receipts / idempotency | `workflow_command_receipt` + `process_commands` (claim-first, replay-safe) |
| append-only history | `process_events`, partitioned by month |

### The artifacts
`contract` + `contract_person` + `contract_firm` + `contract_property`

```sql
contract: contract_type · form_template_id · source_form_instance_id
          predecessor_contract_id · facts(jsonb) · status · executed_at
          evidence_document_id
```

Role vocabulary (one vocabulary, relabelled per form):

```
contract_person    BUYER · SELLER · BUYER_BROKER · SELLER_BROKER · SELLER_SPOUSE
                   BUYER_COUNSEL · SELLER_COUNSEL · CLOSING_NOTARY · LENDER_CONTACT
                   SELLER_REPRESENTATIVE
contract_firm      BUYER_BROKERAGE · SELLER_BROKERAGE · LENDER · TITLE_COMPANY
                   ESCROW_HOLDER · APPRAISAL_FIRM · LAW_FIRM · BUYER · SELLER
contract_property  SUBJECT_PROPERTY          <- "the one being sold"
```

Same role codes, different labels per form — which is why identities ride the chain:
`SHOW-RPT` renders `BUYER` as *Visitor*, `SELLER` as *Property Owner*, `BUYER_BROKER` as
*Showing Agent*.

### The other services (all service-owned as of 2026-09-11)
`Person` · `Property` · `Forms` (13 ops) · `Vault` (12 ops) · `Contract` (6 ops incl. the
bridge) · `Showing` · `Project` · `WBS` — one kernel, `lib/service-runtime.ts`.


---

## 3. The chains, and their caveats (the part that must not get rigid)

```
SELLER   Listing Contract ------> P&S ------> Closing
BUYER    Showing Report (maybe) -> Offer -> (negotiation) -> P&S -> Closing
```

**Caveats, stated by the owner, that the model must honour:**

- A **Showing Report is a minor contract.** "I saw a house, maybe I like it, maybe I never
  come back." It is a complete contract on its own (8 fields; Visitor/Owner/Showing Agent)
  and it may simply stop. That is a valid outcome, not a stuck process.
- An **Offer is real but not valid** until the seller accepts. It is *tentative* while terms
  are negotiated. In model terms: `status='draft'`, `executed_at IS NULL`. "Not valid" is not
  a special case — it is an unexecuted contract.
- **Negotiation produces successive contracts** (offer, counter, counter...), linked by
  `predecessor_contract_id`; the accepted one becomes the P&S. Nothing transforms in place,
  so negotiation history survives.
- The **P&S is the binding, huge contract** — and it is what starts the transaction
  (`pns_preparation` in the supermodel).
- **Therefore: no required sequence.** "I saw a house" must never imply "I must have a P&S."

**What to avoid:** the legacy `deal` stage machine is exactly the anti-pattern —
`DealStage = new_lead -> qualified -> showing -> offer -> under_contract -> closed` with
`ALLOWED_TRANSITIONS` enforced in `db/deal-stage.ts`. One linear funnel, no dead ends, no
branches, no restarts. The Contract model has **no equivalent**, and must not grow one.

**What the model must allow:** dead ends (a showing that stops) · branches (three offers on
three houses at once) · restarts (an offer expires, a new one begins) · optional steps.

---

## 4. Form fields ARE contract scope

Every field in every template lands in exactly one of three buckets:

| bucket | LISTING-01 (12 flds) | PR-PNS (34 flds) | destination |
|---|---|---|---|
| **entity link** | `sellerName`, `brokerName` | `buyerName`, `sellerName`, `buyerBrokerName`, `sellerBrokerName`, `spouseName` | a contract **role** (`person_id`) + `snapshot_name` (frozen at signing) |
| **entity attribute** | `property`, `propertyLocation`, `catastroNumber` | `municipality`, `registryEntry`, `fincaNumber`, `registrySection` | the **Property** record (those columns already exist on `property`) |
| **contract fact** | `listPrice`, `commission`, `startDate`, `endDate`, `listingType` | `purchasePrice`, `deposit`, `cashAtClosing`, `closingDate`, `escrowHolder`, `notaryName`, `effectiveDate`, `financing`, `financingDeadline` | **`contract.facts`** (jsonb) |

So the "denormalized superset" is right — and needs **no schema change**: `facts` is jsonb,
shaped per `contract_type`.

And `snapshot_name` already solves the subtle part: the live link (`person_id`) **and** the
name as signed.

`contract.createFromForm` already takes exactly:

```ts
{ contractType, formTemplateId, sourceFormInstanceId, predecessorContractId,
  propertyId, roles[], facts{} }
```

**The only thing missing is the mapper**: template fields + `<participants>` -> `roles[]` +
`facts{}`. It is a pure function over the template definition, identical for all seven forms,
because they all draw from one vocabulary.


---

## 5. The gaps — precise

**5.1 No corridor between a contract and its process.**
`process_instances.subject_type='deal'` + `subject_id=<deal id>` today. Nothing on `contract`
references a process instance (verified: zero references). So the artifacts and the
transaction that produces them cannot see each other. **This is the cord.**

**5.2 The supermodel starts at the P&S.**
`pns_preparation` is its first state. Origination — showing -> offer -> negotiation -> signing —
has no definition. It needs either a sibling definition for the origination chain, or a
modelled extension upward. The engine supports both (`subject_type` is polymorphic,
`parent_instance_id` nests instances).

**5.3 The legacy `deal` aggregate still owns the transaction identity.**
The engine's business instances are keyed to `deal` (7 PROD rows), while the modern domain is
`contract` (0-1 rows). The UI shows "Deals" (the nav already says *Contracts*); the workspace
page reads `db/deal-workspace`. A `deal` is really the *transaction*; a `contract` is its
artifact. Naming that honestly is half the work.

**5.4 One PROD instance is in `error` (deal/60000000, 2026-08-20).**
A real transaction process failed and was never recovered. Not tonight's problem, but a real
loose end.

---

## 6. Recommended design (the shape to build toward)

**The process is the transaction. The contract is an artifact. Nothing more.**

1. **Subject.** Keep `process_instances.subject_type` polymorphic:
   - `subject_type='deal'` — the transaction (what runs today).
   - `subject_type='contract'` — the origination chain (showing -> offer -> P&S).
   Link the P&S to the transaction instance it starts via `parent_instance_id`.
2. **Add the cord (5.1).** A contract must name its process instance:
   `contract.process_instance_id`. Then "which contracts does this transaction have" and
   "which transaction does this contract belong to" are both a query.
3. **Origination definition.** A second XML definition beside `RE_supermodel-v1.xml`, in the
   same vocabulary: `showing_recorded` -> `offer_drafted` -> `offer_sent` ->
   `awaiting_seller` (human wait) -> `countered` (loop) -> `accepted` -> *hands off to*
   `pns_preparation`. Dead ends are terminal states, not failures: `withdrawn`, `expired`,
   `not_pursued`.
4. **Contract status is per contract**, never a global funnel:
   `draft -> sent -> countered -> accepted/executed -> closed`, plus `withdrawn · expired ·
   rejected · superseded`. A terminal non-success is a *successful* end of that contract.
5. **Human waits are the product.** Every `task-node` becomes visible work — and that is where
   **Projects / WBS** come in: task rows already carry `node_id` + `process_instance_id`;
   WBS items can hang off the same transaction.
6. **Evidence at every signing.** When a state requires a signature, the contract is issued
   into the **Vault** and `contract.evidence_document_id` points at it. The transaction's
   history is then: states (engine) + artifacts (contracts) + evidence (vault).
7. **Jurisdiction stays a fact.** PR/Culebra vs FL differences remain facts
   (`requiresNotario`, `closingAgentRole`, ...) — never node names, never branches in code.

---

## 7. Build order when this resumes

1. **The mapper** — `template + participant bindings -> { roles[], facts{} }`. Pure, testable,
   no schema change. It makes `contract.createFromForm` real instead of theoretical.
2. **The cord** — `contract.process_instance_id` + the queries on both sides.
3. **Origination definition** — the second XML graph, reusing the vocabulary.
4. **WBS tie-in** — task-nodes -> work items on the transaction.
5. **The Deals->Contracts page** — `contract.list` (built) instead of `db/deals`.
6. **Reconcile `deal`** — decide whether the transaction aggregate stays `deal` (with contracts
   attaching to it) or migrates. Rename nothing until 1-5 are done.

---

## 8. Non-negotiables

- Optional, progressive, tentative, reversible. No `ALLOWED_TRANSITIONS` funnel.
- One role vocabulary; forms relabel, never extend.
- Facts are facts; jurisdiction is never a branch.
- Forms are views onto the contract, not the domain.

---

## 9. The open question: is there a parent contract, or are contracts peers?

**Answer: neither. There are three different things, and the confusion comes from
trying to make one of them do the others' job.**

```
TRANSACTION  (the matter / file)          <- the container. NOT a contract.
    │  property · parties · progress · documents · work · the closing outcome
    ├── Contract  (Listing Agreement)      <- peer, own lifecycle
    ├── Contract  (Offer)                  <- peer, own lifecycle, may die alone
    └── Contract  (P&S)                    <- peer, own lifecycle  ← the binding one
             ▲                            ▲
             │ lineage                    │ lineage
        Listing Agreement            accepted Offer
```

### 1. The transaction is the "master" — and it is not a contract

This is the role `deal` plays today (`process_instances.subject_type='deal'`). It is the
container that owns: the property, the parties, the progress, the closing outcome, the
documents, and the work items. It has **no legal force** — nobody signs the transaction.

Making a *contract* the parent would be wrong: the parent would need to exist before any
child (but a showing report can exist with no listing), and it would imply composition
(delete the parent -> delete the children), which is unthinkable for executed instruments.

### 2. Contracts are peers, never children

Each contract is a complete legal instrument: its own parties, its own facts, its own
`status`, its own `executed_at`, its own evidence. The showing report that goes nowhere, the
offer that expires, the P&S that binds, and the listing that started it are the same *kind*
of thing. None is "part of" another.

### 3. Lineage is a DAG of typed links — because the P&S is a JOIN

This is the part that a single `predecessor_contract_id` cannot express:

```
Seller side:  Listing Agreement ─────┐
                                     ├──► P&S ──► (closing)
Buyer side:   Showing ─► Offer ──────┘
```

The P&S has **two** predecessors — it joins the seller's chain and the buyer's chain. That is
exactly a **join**, and the engine already models joins (2 `join` nodes in the supermodel).

So:

- `predecessor_contract_id` stays for the **simple linear case** (showing -> offer ->
  counter -> offer). 1:1, cheap, already there.
- When the join matters, use a **typed link** (`contract_link`: from, to,
  `converted_to` | `supersedes` | `amends`). M:1 into the P&S, 1:1 along a chain.
- `status` on each contract (`draft · sent · countered · accepted · closed`, plus
  `withdrawn · expired · rejected · superseded`) says which branch died and which lived.
  No extra "winner" flag is needed — the accepted one is `executed`.

### 4. Cardinality, plainly

| relation | cardinality |
|---|---|
| transaction -> contracts | 1 : M (attachment) |
| contract -> next step (linear) | 1 : 1 (`predecessor_contract_id`) |
| chain into the P&S | M : 1 (a **join**; needs typed links) |
| contract -> parties | 1 : M roles (`contract_person` / `contract_firm`) |
| contract -> property | M : 1 (`contract_property.SUBJECT_PROPERTY`) |
| contract -> evidence | 1 : 1 (`evidence_document_id` in the Vault) |

### 5. Recommendation

1. Keep **`predecessor_contract_id`** for linear lineage — do not remove it.
2. Add **`contract_link`** (typed, M:N) only when the P&S join is built. Do not build it
   before there is a join to record; a link table with one edge type is premature.
3. Treat the **transaction** (`deal` today) as the container and never let a contract be a
   container of contracts.
4. Never cascade-delete a contract. Executed instruments are evidence.

That gives you the "master" you were reaching for (the transaction), keeps each contract
independent and legally honest, and lets the P&S be what it actually is — **a merge**, not a
child.

- Everything through the service kernel; nothing screenside touches tables.
