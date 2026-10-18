# Executive Expert Council

**Layer:** Escalation / decision support  
**Status:** Phase 1 active  
**Purpose:** Resolve important multi-domain decisions without turning normal engineering into committee work.

## Core seats

- **Chief of Staff** — moderator, context, decision capture, owner-attention filter.
- **Advisor** — strategy, trade-offs, sequencing, second-order effects.
- **Principal Architect** — architecture, ownership, technical consequences.
- **Adversary** — mandatory dissent seat; attacks the favored answer and hidden assumptions.

Add specialists only when their domain materially affects the result: Security, Database, DEV_OPS, Researcher, implementation owner, or another subject expert.

## Trigger

Council is warranted only when all are true:

1. at least two credible paths remain;
2. the consequences are material; and
3. reversal is expensive, risky, or strategically costly.

Examples:

- major platform/vendor migration;
- new system architecture with long-lived lock-in;
- security architecture;
- build-vs-buy with material operating cost;
- destructive or difficult data migration strategy;
- infrastructure move that changes operating model;
- two valid architectures with different product/business consequences.

Do **not** invoke Council for:

- ordinary bug fixes;
- routine reversible refactors;
- clear-domain implementation choices;
- decisions already settled by a binding architecture/policy rule;
- a desire for more opinions after sufficient evidence already identifies the answer.

## Three rounds

### Round 1 — Independent positions

Each voting expert reasons independently before seeing the others' conclusion.

Required fields:

```text
POSITION
RECOMMENDATION
CONFIDENCE
PRIMARY EVIDENCE
PRIMARY RISK
```

This prevents early consensus from becoming the evidence.

### Round 2 — Cross-examination

Each expert challenges the strongest opposing position.

Focus on:

- unsupported assumptions;
- missing data;
- second-order effects;
- reversibility;
- operational failure modes;
- security/data integrity;
- cost of being wrong.

Do not repeat Round 1 in different words.

### Round 3 — Decision

The moderator records:

```text
DECISION / RECOMMENDATION
CONFIDENCE
WHY
DISSENT
RISKS ACCEPTED
REVERSIBILITY
OWNER DECISION REQUIRED? yes|no
NEXT ACTION
```

Dissent is mandatory when a seat still disagrees. Do not erase it to create artificial unanimity.

## Authority

Council is advisory unless an existing policy explicitly delegates the decision.

Council does **not**:

- change Forge workflow state directly;
- grant a role new authority;
- override `AGENTS.md`, QA boundaries, security boundaries, or production authorization;
- publish/deploy;
- edit code as a committee.

If owner authority is required, route the executable workflow to the canonical HOLD/human-resolution path with the Council recommendation attached as evidence.

## Connection to Forge

Council sits **above** the autonomous SDLC. Its result becomes input to the existing system:

- cross-project durable decision -> canonical `MEMORY.md` / architecture documentation;
- one-story implementation decision -> story packet / architecture contract;
- unresolved owner decision -> workflow HOLD;
- executable accepted work -> normal Forge Story Board / XML path.

Never add Council as a mandatory node to every `FORGE_SDLC` execution.

## Adversary requirement

For a Council to call itself complete, the favored option must survive one explicit adversarial pass.

The Adversary asks:

- Why is this conclusion wrong?
- What assumption are all other seats sharing?
- What fails under partial outage or degraded mode?
- What hidden coupling or lock-in appears later?
- What would make the rejected alternative win?
- What evidence is being treated as fact but is actually inference?

The Adversary does not get a permanent veto. It makes the cost of dissent visible.

## Decision record

Council output must be durable when the decision affects future work. Store it in the canonical owner rather than creating a separate Council database:

- `MEMORY.md` for durable repo-wide decisions;
- the story packet / durable workflow evidence for story-scoped decisions;
- HOLD resolution for a human decision required to continue.

## Cost discipline

Council is intentionally expensive. Use one expert when one domain can settle the question. The presence of Council in Phase 1 is an escape hatch for hard decisions, not a default routing strategy.