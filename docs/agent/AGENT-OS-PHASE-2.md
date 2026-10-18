# Agent OS — Phase 2 Controlled Expert Swarm

**Status:** Phase 2 contract landed; harness integration required  
**Authority:** CTO / Product Owner  
**Runtime contract:** `forge/src/engine/executive_swarm.rs`

Phase 2 turns the Phase 1 executive layer into a bounded executable reasoning system while preserving one architectural rule:

> **The swarm decides. Forge executes.**

No executive or expert swarm role receives application-code, publication, migration, deployment, or production mutation authority. The seven existing Forge role services and `FORGE_SDLC-v6.xml` remain the software-delivery machine.

## 1. Topology

```text
CHRIS
  |
  v
CHIEF OF STAFF
  |  classify + coordinate + synthesize
  |
  +-- ADVISOR
  +-- PRINCIPAL ARCHITECT
  +-- ADVERSARY (Council / challenge)
  |
  +-- EXPERT POOL
        security
        database
        rust
        ui-mvi
        devops
        integration
        performance
        research

parallel independent verdicts
          |
          v
Chief of Staff synthesis
          |
          +-- decision only / record
          +-- HOLD / Chris when owner authority is required
          +-- ForgeDecisionHandoff -> existing Story Board / Forge SDLC
```

The Chief of Staff is coordinator and synthesizer, not an expert vote and not an implementation role.

## 2. Stable executable IDs

The canonical identifiers are typed in `engine/executive_swarm.rs`:

### Executive

- `exec-chief-of-staff`
- `exec-advisor`
- `exec-principal-architect`
- `exec-adversary`

### Experts

- `expert-security`
- `expert-database`
- `expert-rust`
- `expert-ui-mvi`
- `expert-devops`
- `expert-integration`
- `expert-performance`
- `expert-research`

The harness must consume these identifiers rather than re-author a competing list.

## 3. Authority

Every Phase 2 executive and expert has **ReadOnly** authority.

They may:

- inspect repository state made available to the harness;
- inspect architecture and current decision records;
- research and compare;
- reason about alternatives;
- challenge assumptions;
- produce a structured verdict;
- use explicitly bounded read-only micro-agents when the harness allows it.

They may not:

- edit application source;
- alter workflow state directly;
- commit, publish, merge, push, or deploy;
- apply migrations;
- mutate production;
- weaken security or entitlements;
- turn a recommendation into execution outside Forge.

This is an enforcement requirement, not merely prompt language.

## 4. Deterministic routing

`plan_swarm()` is the canonical routing contract.

Examples:

| Signal | Panel |
| --- | --- |
| Strategy | Advisor |
| Architecture | Principal Architect |
| Security | Principal Architect + Security |
| Database | Principal Architect + Database |
| Rust | Principal Architect + Rust |
| UI/MVI | Principal Architect + UI/MVI |
| Deployment | Principal Architect + DevOps |
| Integration | Principal Architect + Integration |
| Performance | Principal Architect + Performance + Rust |
| Research | Research |
| Build vs buy | Advisor + Principal Architect + Research |
| Platform | Advisor + Principal Architect + DevOps |
| Cross-domain | Advisor + Principal Architect |

Signals may be combined. Duplicate seats are removed deterministically.

The router **must not silently truncate** an over-broad panel. If the selected seats exceed the mode's cap, the request fails and the Chief of Staff narrows or decomposes the decision.

## 5. Swarm vs Council

### Advisory swarm

Use for a complex question that benefits from multiple independent specialties but does not warrant a full Council.

Maximum expert seats: **5**.

### Council

Council is allowed only when all are true:

1. at least two credible paths remain;
2. consequences are material;
3. the decision is meaningfully difficult to reverse.

Maximum seats: **6**.

Council always adds `exec-adversary` as the dissent seat.

Chief of Staff moderates and records. It does not count as an expert seat.

## 6. Independence

Each expert receives the same bounded decision packet and forms its initial verdict independently.

Do not let one expert's answer prime the next expert before the independent pass.

The minimum packet is:

```text
QUESTION
GOAL
CURRENT ARCHITECTURE / KNOWN STATE
CONSTRAINTS
KNOWN FACTS / EVIDENCE
DECISION REQUIRED
OUT OF SCOPE
```

## 7. Expert verdict contract

The Rust contract is `ExpertVerdict`.

Each result must contain:

```text
AGENT
VERDICT: PROCEED | REJECT | CONDITIONAL
RECOMMENDATION
CONFIDENCE: 0..100
KEY REASONS
PRIMARY RISK
ASSUMPTIONS
EVIDENCE
WHAT WOULD CHANGE MY MIND
```

A verdict with confidence above 100, no recommendation, or no primary risk is invalid.

## 8. Synthesis

Synthesis is not majority voting.

Chief of Staff must determine:

- where experts agree;
- where they disagree;
- whether disagreement is factual or judgmental;
- which assumptions create the disagreement;
- which specialist has the relevant domain authority;
- what the Adversary exposed;
- whether new evidence is actually necessary.

The durable synthesis shape is `DecisionSynthesis`:

```text
DECISION ID
QUESTION
RECOMMENDATION
CONFIDENCE
CONSENSUS
DISSENT
ACCEPTED RISKS
NEXT ACTION
SOURCE AGENTS
```

Dissent is preserved. It is not erased merely because the final recommendation goes another way.

## 9. Forge handoff

The only Phase 2 shape intended to cross into implementation is `ForgeDecisionHandoff`.

It carries:

- optional Story Board id;
- decision id;
- objective;
- recommendation;
- architecture contract;
- acceptance criteria;
- dissent to preserve;
- source agents.

It carries **no execution authority**.

After synthesis:

```text
executive/expert swarm
       |
       v
ForgeDecisionHandoff
       |
       v
Story Board / existing Forge workflow
       |
       v
Scout / Architect / Lead / Smith / Inspector / Assay / DEV_OPS
```

Phase 2 must not create an alternate Smith, QA path, publisher, queue, workflow graph, or release path.

## 10. Autonomous escalation

The typed escalation ladder is:

```text
Execution sequencing       -> Lead
Architecture boundary      -> Principal Architect
Strategic trade-off        -> Advisor
Multi-domain irreversible  -> Council
Product intent             -> Chris
Production authorization   -> Chris
```

Use the lowest level capable of resolving the issue.

A Forge role that encounters an executive-level issue should preserve its existing workflow state/HOLD semantics, obtain the executive decision through the integration seam, then resume through the existing HOLD resolution or story contract. The model does not invent a new state transition.

## 11. Fan-out controls

Hard contract:

- advisory expert seats: max 5;
- Council seats: max 6;
- child questions per primary expert: max 3;
- child depth: max 1;
- no grandchildren;
- Chief of Staff is one coordinator/synthesizer;
- experts are read-only;
- no background child mode until the harness can prove join + spend accounting.

Where existing OpenCode controls are stricter, the stricter bound wins unless the Captain explicitly changes it.

## 12. Harness integration rule

The current OpenCode harness already has its own role profiles, permission renderer, skills, micro-subagents, child limits, environment delivery and regression rails.

**Do not replace that machinery.**

DeepSeek's integration task is to reconcile the Phase 2 contract into the current harness so that:

1. the IDs above become actual selectable OpenCode execution profiles;
2. all are read-only at the tool-permission layer;
3. their skills are mapped from the existing canonical skill catalogue where possible;
4. any new skill is authored once in `docs/agent/skills/` and generated through the existing renderer;
5. executive/expert children obey the existing subagent arming and cost-accounting policy;
6. `plan_swarm()` remains the routing authority for which experts belong on a panel;
7. the harness executes a plan; it does not recreate routing policy;
8. Forge XML is not changed merely to host the executive layer;
9. decision output is persisted through the existing durable decision/HOLD/artifact architecture, not chat alone;
10. tests prove the new profiles cannot edit or publish.

## 13. Acceptance rails

The Phase 2 contract already includes Rust unit tests for:

- every executive/expert is read-only;
- Security routes to Principal Architect + Security;
- Build-vs-buy routes to Advisor + Principal Architect + Research;
- Council refuses a request that fails its three-part gate;
- Council always includes Adversary;
- an over-broad panel fails rather than silently dropping a seat;
- child fan-out/depth remain bounded;
- invalid verdict confidence is rejected;
- escalation target is deterministic;
- Forge handoff preserves dissent and grants no executive write authority.

Harness integration must add its own enforcement rails in the existing OpenCode test surface.

## 14. Definition of Phase 2 done

Phase 2 is complete when all of these are true:

- the four executive IDs and eight expert IDs resolve to real harness profiles;
- all twelve are read-only in actual tool enforcement;
- Chief of Staff can execute a typed `SwarmPlan`;
- experts can run independently from the same packet;
- results validate as `ExpertVerdict`;
- Chief of Staff produces `DecisionSynthesis`;
- Council enforces its gate, seat cap and mandatory Adversary;
- bounded micro-agent use respects the existing harness's accounting/arming constraints;
- decision artifacts persist durably;
- a `ForgeDecisionHandoff` can be handed to the existing SDLC without changing XML ownership;
- no swarm role can write application code, publish, deploy, migrate, or mutate production;
- existing Forge role, QA and DEV_OPS boundaries remain green.
