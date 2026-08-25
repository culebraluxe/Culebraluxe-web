# ARCH-01 README Supplement — Durable Architecture Continuity

**Status:** Active architecture operating rule  
**Effective:** 2026-08-24  
**Purpose:** Preserve enough verified technical state for a new architect or
agent session to continue CulebraLuxe safely without reconstructing the project
from chat history.

This supplement extends ARCH-01. It does not replace the canonical Story Board,
`AGENTS.md`, repository code, migrations, or story-specific architecture
documents.

## 1. Continuity principle

Chat context is working memory, not durable architecture.

A handoff is complete only when the current technical continuity state is:

1. written to a versioned file in Git;
2. committed with an identifiable commit SHA;
3. reachable through this stable supplement or a stable current-continuity
   pointer; and
4. reconciled by the incoming architect against the actual repository and
   database before new implementation begins.

Do not use a pasted transcript, model memory, or an agent's final report as the
sole authority for continuing engineering work.

## 2. Stable entry point

Every incoming architect must begin with:

1. `AGENTS.md`
2. this file: `docs/ARCH-01-README-SUPPLEMENT.md`
3. the current technical continuity packet referenced below
4. the applicable story/architecture specification
5. the current Story Board rows
6. current Git status, branch topology, and relevant code

Current continuity packet:

- `docs/final-technical-continuity-packet-2026-08-24.md`
- commit `98a50601f89fc4d12fdc6d15e0e2347b4b67534d`

Current Deal workflow architecture packet:

- `docs/deal-workflow-architecture-specs-2026-08-24.md`
- commit `92d271d051e3fb8ec121f3eeabc9f7de5f2c0c6e`

When a later packet supersedes the current packet, update this stable section
in the same commit that adds the replacement packet. Keep prior dated packets
as historical evidence unless they contain secrets.

## 3. Mandatory reconciliation rule

The continuity packet is continuation state, not unquestionable truth.

Before proposing or changing implementation, the incoming architect must:

1. inspect the repository and relevant migrations;
2. inspect the current Story Board and database schema when the connector is
   available;
3. identify the precise Git commit/branch containing candidate work;
4. distinguish migrations committed in Git from migrations actually applied;
5. verify named classes, functions, tables, commands, events, and tests exist;
6. call out every contradiction between the packet and current implementation;
7. preserve established canonical boundaries unless a reviewed architecture
   decision explicitly changes them; and
8. stop rather than invent business semantics or missing authority.

An agent must not silently redesign around a stale handoff.

## 4. Required continuity-packet contents

Each replacement technical continuity packet must include the following.

### 4.1 Authority and decisions

- date and scope of the packet;
- authoritative architecture documents and commit SHAs;
- resolved business/architecture decisions;
- unresolved decisions and explicit stop conditions;
- which artifact wins if two documents conflict.

### 4.2 Git and working state

- current branch and HEAD commit;
- relevant commit ancestry, merge bases, and divergence;
- candidate/review branches and unmerged commits;
- known local uncommitted work, when observable;
- files or workstreams that another agent must not touch;
- required commit boundaries and whether pushing is authorized.

Never assume that two known commits are parent/child. Verify their topology.

### 4.3 Database state

- Neon project, database, and branch identity when verified;
- migrations present in Git;
- migrations confirmed applied in the target environment;
- schema objects relevant to active work;
- canonical Story Board rows and execution status;
- connector/authentication limitations;
- clear separation of DEV permission from production prohibition.

Do not report a migration as active merely because its SQL file exists.

### 4.4 Existing architecture seams

- canonical domain ownership boundaries;
- command, receipt, transaction, and event contracts;
- outbox/MQ/integration-inbox boundaries;
- workflow application ports and correlation seams;
- read models and presentation projections;
- provider adapters and provider-neutral contracts;
- exact file paths and exported entry points that must be reused.

The packet must explicitly identify any existing seam that prevents an agent
from creating a duplicate model, queue, workflow, transport, or state store.

### 4.5 Current implementation truth

- stories actually complete versus partially implemented;
- exact commits and files changed;
- known architectural defects;
- incomplete runtime wiring hidden behind passing unit tests;
- idempotency, retry, recovery, and transaction-boundary behavior;
- any claimed end-to-end proof and what it actually demonstrated.

`shouldEmit = true`, a TypeScript union member, or a passing predicate test is
not proof that a durable business event was published.

### 4.6 Verification

- targeted tests completed;
- broader regression checkpoints completed or still pending;
- typecheck/build results when relevant;
- DEV database or E2E proofs actually exercised;
- known test harness limitations;
- the next meaningful regression checkpoint.

Do not convert “full regression at an architectural checkpoint” into “full
site regression after every edit.”

### 4.7 Active work and next order

- active agent/session and exact assigned scope;
- the work order given to that agent;
- files or stories currently under active modification;
- the next bounded implementation order;
- acceptance criteria, test scope, commit boundaries, and stop conditions;
- work that must not begin until the active result is reviewed.

## 5. When continuity must be checkpointed

Update the continuity packet at the earliest applicable point:

- after a major architecture decision;
- after a tightly coupled story batch;
- after discovering a non-obvious canonical seam or architectural defect;
- before intentionally ending a long architecture/build session;
- when context capacity is becoming constrained;
- before handing work to a different model, agent, or human engineer;
- after Git or database topology changes materially;
- after an implementation report contradicts the actual code;
- after a failed connector prevents canonical Story Board updates.

Do not wait until the final chat turn if important continuity state has already
changed.

## 6. Handoff creation procedure

1. Review current Git, code, tests, Story Board, and database state.
2. Write a dated technical packet under `docs/`.
3. Separate verified facts from memory or unverified connector-dependent state.
4. Include exact paths, symbols, tables, commits, and transaction boundaries.
5. Include the next bounded work order rather than only narrative history.
6. Commit the packet.
7. Update the stable pointer in this supplement to the new packet and commit.
8. Give the incoming architect the stable filename, not a giant pasted packet.
9. Require the incoming architect to reconcile the packet against reality.

If Git is available but the Story Board connector is not, Git is the durable
fallback. Record the pending Story Board update in the packet and reconcile it
when database access returns.

## 7. Incoming-session startup procedure

An incoming architect should be able to start with one instruction:

> Read `docs/ARCH-01-README-SUPPLEMENT.md`, follow its current continuity
> pointer, and reconcile the packet with the repository and database before
> doing further work.

The incoming architect then reports only:

- what it verified;
- what contradicted the packet;
- what remains inaccessible or unverified;
- the bounded work it is now prepared to perform.

It should not ask the user to reconstruct the prior week unless a material fact
is absent from both Git and the Story Board.

## 8. Failure and conflict handling

If a continuity source conflicts with another source, use this precedence only
after reporting the conflict:

1. explicit current human decision;
2. actual current canonical code and database behavior;
3. current Story Board/architecture record;
4. committed architecture specification;
5. committed technical continuity packet;
6. agent report or chat memory.

This precedence does not authorize silent redesign. A contradiction affecting
business semantics, system-of-record ownership, or transaction guarantees is a
stop condition requiring review.

If Neon or another connector fails before SQL execution, record the connector
failure separately from database health. Do not describe an OAuth/plugin error
as a database outage.

## 9. Security and data hygiene

Continuity packets must never contain:

- database passwords or connection strings;
- API keys, webhook secrets, or OAuth tokens;
- private signing payloads;
- personal contact exports or unnecessary client data;
- production-only credentials;
- raw environment-file contents.

Reference configured secret names and environment ownership, never secret
values.

## 10. ARCH-01 completion standard

ARCH-01 continuity is functioning when a new architect can, without relying on
prior chat history:

1. identify the authoritative architecture;
2. identify the current Git and database state;
3. locate and reuse canonical seams;
4. distinguish completed work from partial implementation;
5. understand active stop conditions and unresolved decisions;
6. issue the next bounded work order safely; and
7. continue without making the user reteach the project.

The objective is not maximal documentation. It is a small, durable,
implementation-specific control surface that makes architectural continuity
routine.


<!-- SESSION-CONTINUITY-2026-08-24-PORTAL-GMAIL-WORKMODE -->
## 11. Session continuity checkpoint — Portal, relationship intake, and operating model (2026-08-24)

This checkpoint preserves current product judgment and implementation state in case
the active ChatGPT Work/Codex session becomes unavailable. Reconcile all volatile
facts against Git, Production, and the Story Board before acting.

### 11.1 Human authority and collaboration contract

- Chris is CTO and Product Owner. Material product, information-architecture,
  navigation, Story Board, or system-boundary changes require explicit approval.
- Treat Chris as a peer senior data architect, not as a novice. Relevant background:
  roughly 30 years building high-scale data systems across Lotus/IBM, JPMorgan,
  State Street, BlackRock, and Royal Ahold; global Security Master, ETF publishing,
  CUSIP/tax-lot/real-time pricing, corporate actions, and a rules-based pricing
  engine for all grocery items across approximately 1,200 stores.
- Current delivery mode is startup/high-risk tolerance: direct small commits to
  `main`, Production as the practical QA surface, rapid fix-forward, and no branch,
  PR, or release ceremony unless Chris explicitly asks for it.
- Deep reviews are read-only by default. Recommendations are proposals. Do not turn
  exploratory discussion into stories, navigation changes, or implementation
  without approval.
- Product north star: a bespoke “Pagani, not Ford” operating tool for Lisa, a solo
  luxury broker. Deep machinery, narrow interface. Automatic capture should protect
  selling time; Lisa must not become a database-maintenance operator.

### 11.2 Lisa-facing information architecture

The working lens is:

- Dashboard = Orient
- Attention/Catch-Up = Act
- Client = Remember
- Deal/Contract = Execute

This is a lens, not a normalized taxonomy. The same canonical fact may be shown in
multiple useful contexts when it prevents a context switch or helps Lisa act.
Remove repetition only when two copies answer the same question in the same
context. Canonical state remains normalized; Lisa’s read experience may be
deliberately denormalized.

Do not revive separate Activity, Showings, or similar destinations merely because
those capabilities exist. Activity can be an evidence ledger inside an operating
surface; Showings can be commitments inside the relevant context. A panel earns
its place when it materially improves another panel, supports a frequent decision,
prevents a costly screen switch, stays current automatically, or justifies its
visual and maintenance cost.

Current approved CORE display vocabulary, while preserving the internal
`NEXUS` identifier:

- Cockpit → `/portal/dashboard`
- Clients → `/portal/clients`
- Catch-Up → `/portal/attention`
- Contracts → `/portal/deals`
- Cabinet → `/portal/documents`

OPPS, SUPPORT, and TECH retain their names. Workflows and Forms remain functional
but are hidden from the visible CORE navigation. The portal logo returns to the
public site; the misleading MAIN top-nav item is removed. The public-site Portal
link is the far-right final navigation item.

### 11.3 Current Git/portal checkpoint

Observed `main` HEAD at the time of this checkpoint:

- `f3bb41009b17eb41af9e60131d97bf8930c9afc6` — media-count casts,
  CORE navigation, capsule top navigation, property-admin 2xl table breakpoint,
  rounded panels, and public Portal link.
- Parent UI work:
  - `da9d80246ecbc7ae1e793b3dc2d6830aaea2da97` — interaction-component facelift.
  - `83228eb490503803f3edfccbaee6ce78cfeb1ea5` — UI Lab portal primitives.
  - `72d72031d8bca9f9d9977f22f89a7f306eef8c23` — Marlowe Gmail census artifacts.
  - `5c9796596f1fa0057d90a96db4360ccb9b62433c` — Attention/showings/activity,
    navigation declutter, glass polish, and Security rename.

The facelift intentionally promoted the custom portal component family only where
it materially helped: tables, pagination, search input, dialogs, and responsive
table-to-card layouts. It did not replace the custom Grok-built glass visual
language or introduce another component dependency.

Known QA findings addressed by `f3bb410` included Postgres filtered-count values
being concatenated as strings in media projections and the property inventory
table becoming cramped before very wide desktop widths. Always verify current
Production rather than assuming deployment state from this checkpoint.

### 11.4 Marlowe Gmail evidence and intake boundary

Committed artifacts:

- `docs/marlowe-gmail-relationship-census-report-2026-08-24.md`
- `docs/marlowe-gmail-relationship-census-private-2026-08-24.csv`

Marlowe proved only partial metadata coverage: 7,696 messages from 2011-06-26
through 2013-12-31 and 2,018 external identities. Counts included 115 two-way,
60 owner-initiated outbound-only, 1,843 inbound-only, and 1,140 with automated or
bulk evidence. The Gmail connector produced nondeterministic/incomplete bounded
results, so this is not a full-history census. No message bodies, subjects,
attachments, or retained snippets were included.

The correct next architecture is not connector-side reconciliation:

source metadata extraction
→ source-faithful ODS/load layer
→ cleansing and classification
→ identity reconciliation
→ relationship mart or carefully governed canonical promotion.

Highest-value first-pass fields are person/name, email, phone,
iMessage/WhatsApp reachability when available, source/provenance, and inexpensive
relationship metadata such as first/last contact and direction/counts.
Organization is weak enrichment. Most remaining Google metadata is stale or noise.
A later, targeted second pass may summarize threads only for high-value
relationships; do not start with broad body ingestion or summarization.

Apple Contacts currently has 2,573 staged imported contacts visible through the
Clients surface without promoting them into canonical `person` or
`person_identity`. Preserve that staging/canonical boundary.

### 11.5 Frozen assumptions and next-session bootstrap

- Do not rename or reorganize surfaces merely to eliminate overlap.
- Do not copy competitor information architecture. Follow Up Boss and Cloze are
  idea mines, not authorities.
- Keep operational/admin CRUD away from Lisa’s daily flow; occasional OPPS data
  stewardship can be designed later.
- The Deal/Contract workspace deserves a separate focused composition review; do
  not mix it casually into portal polish.
- Before new work: read this file, query the live `ARCH-HANDOFF` Story Board row,
  inspect current `main` HEAD and Production, and reconcile any active builder
  report. Then state observed facts, contradictions, and the smallest proposed
  next move.
- If this checkpoint conflicts with a later explicit decision from Chris, the
  later human decision wins and this file must be updated rather than silently
  interpreted around it.

## 12. REL-INTEL — source-neutral relationship evidence foundation

Implemented as the "relationship-intelligence foundation" slice (migration
`074_relationship_evidence.sql`, commit SHAs in the Story Board/ARCH-HANDOFF
record). This PROVES the ODS pipeline against the partial Gmail census; it is
NOT a full-mailbox census and must not be treated as one.

### 12.1 Neutral seam

`integration_relationship_evidence` is one current-state row per
`(source, source_account, source_identity_key)`, holding identity evidence
(display name, org, labeled emails/phones), communication evidence (nullable —
never invented), bounded-coverage notes, and command-owned canonical
reconciliation (match method/confidence/review state/rule version).

Provenance points to the existing immutable `integration_intake_batch` and
`integration_staged_contact_profile` rows. No new inbox, queue, event store, or
promotion model was created.

### 12.2 Sources

- Apple: the existing 2,573 staged contacts are projected from `l_person` +
  `l_person_identity` into the neutral seam via `lib/relationship-intel/`
  (`apple-projector.ts` pure mapper + `apple-evidence.ts` DB loader). Apple
  contacts are NOT converted into canonical Clients.
- Gmail: the partial census artifact (2011-06-26 → 2013-12-31, 2,018
  correspondents) loads via `lib/relationship-intel/gmail-census.ts` (parser)
  + `gmail-loader.ts` (orchestrator). Aggregate evidence only — no bodies,
  snippets, or attachments. Coverage bounds live in batch/coverage metadata.

### 12.3 Normalization and replay

- `normalize.ts`: deterministic email (trim/lowercase) and US/Puerto Rico phone
  (digits-only, 10-digit reliable; ambiguous international numbers quarantined).
  Original values always retained; invalid values quarantined, not deleted.
- Deterministic `fingerprint` for replay safety; replaying a batch upserts the
  same row (idempotent) and changed payloads are distinguishable.
- Spreadsheet-formula injection is neutralized on CSV cell parse.

### 12.4 Reconciliation

`reconcile.ts` is deterministic and explainable: explicit source link > exact
normalized email > exact normalized phone > review candidate > unmatched/
deferred. Weak fuzzy-name similarity is NEVER an automatic match. Automated/
bulk and service/organization senders are suppressed (rejected / non_person).
Canonical Persons are never silently merged; ambiguity is a reviewable outcome.
Canonical linkage only happens through the existing command/receipt seam after
human approval (`recordReconcileDecision` writes the review fields, not
canonical tables).

### 12.5 Read model and product activation

- `db/relationship-evidence.ts` exposes a per-Person read model
  (`getRelationshipEvidenceForPerson`) and a server-side filtered OPPS review
  (`getRelationshipEvidenceReview`), plus an API route
  `/api/portal/relationship-evidence-review`.
- Client Dossier now shows a compact "Relationship memory" section when the
  seam is populated; it is defensive (renders nothing if unavailable) and bulk
  mail is never treated as fresh contact.

### 12.6 Known limitations and next dependency

- The Gmail artifact is partial and the connector is unreliable for full-census
  enumeration. Production-ranking on the partial set would be misleading, so
  Catch-Up consumes evidence conservatively.
- A deterministic full Gmail census requires a message-ID manifest and
  guaranteed metadata-only batch reads; that is a separate future dependency,
  NOT part of this slice.
- The `074_relationship_evidence.sql` migration must be applied in the DEV
  database and (when Chris authorizes) Production before the neutral seam or the
  OPPS review surface can return real rows.
