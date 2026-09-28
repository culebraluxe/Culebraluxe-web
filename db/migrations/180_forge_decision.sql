-- 180_forge_decision.sql
--
-- PHASE 2 OF THE FACTORY (ENG-FORGE-FACTORY-01, Object 2): a DECISION INSTITUTION — the store that
-- outlives the agent that learned the lesson.
--
-- WHAT IT IS FOR: `MEMORY.md` is an incident narrative written for a human. A rule that must be true
-- for every future lane cannot live there: an agent reads it as one more document among many, and a
-- decision that only lives in prose is not in force. So each decision is ONE ROW, and the lane runner
-- INJECTS the active ones into Lead/Smith context before they act. The packet's words: this replaces
-- "read MEMORY.md and summarize".
--
-- ONE SENTENCE PER ROW is enforced, not requested. The packet's first stop condition is "the decision
-- table becomes a blog" — and the shape that produces a blog is a statement with three paragraphs in
-- it. The CHECK below refuses a second sentence, so the pressure is applied where the writing happens
-- rather than in a review later.
--
-- WHO MAY WRITE is application policy, not schema policy (`lib/forge-decision.ts`), because it is a
-- question about ROLES, not about rows: Scout may insert a candidate and nothing else, Architect and
-- the captain promote, Smith may only read. The table keeps the audit trail those rules need
-- (`owner`, `promoted_at`, `supersedes_id`, `source`).
--
-- Non-destructive: one new table. No existing row or column is touched.

create table if not exists forge_decision (
    id uuid primary key default gen_random_uuid(),
    -- Stable slug, e.g. `batch-table-is-job-stream`. This is how a decision is referenced in a packet,
    -- a run note or a git mirror filename, so the key is the identity and the id is plumbing.
    key text not null unique,
    -- One sentence, present tense, no hedging. `state is in rows` beats `we should probably...`.
    statement text not null,
    status text not null default 'candidate',
    -- Human or lane that promoted it. Required by the promotion-shape check below.
    owner text,
    -- What proved it: a git sha, a probe name, a gate. Nullable, because a seeded invariant proved by
    -- a practice (a launchd wrapper flag) has no single commit.
    evidence_sha text,
    supersedes_id uuid references forge_decision(id),
    source text not null default 'packet',
    domain text not null default 'forge',
    created_at timestamptz not null default now(),
    promoted_at timestamptz,
    superseded_at timestamptz,
    constraint forge_decision_status_check
        check (status = any (array['candidate'::text, 'active'::text, 'superseded'::text])),
    constraint forge_decision_source_check
        check (source = any (array['packet'::text, 'hunter'::text, 'incident'::text, 'captain'::text])),
    constraint forge_decision_domain_check
        check (domain = any (array['forge'::text, 'crm'::text, 'web'::text, 'ops'::text])),
    -- An active decision must name who promoted it and when; a candidate must not pretend either.
    -- Without this, "active" becomes a status anyone can set by omission.
    constraint forge_decision_promotion_shape_check
        check (
            (status = 'active' and owner is not null and promoted_at is not null)
            or (status <> 'active' and promoted_at is null)
        ),
    -- ONE SENTENCE: no sentence-ending punctuation followed by more text, and a length cap that makes
    -- a paragraph impossible even without punctuation abuse.
    constraint forge_decision_one_sentence_check
        check (statement !~ '[.!?]\s+\S' and length(statement) <= 240)
);

-- The injector's query, exactly: active decisions for one domain, newest promoted first.
create index if not exists forge_decision_active_idx
    on forge_decision (domain, promoted_at desc)
    where status = 'active';

-- Candidates queue for promotion; the cockpit will want this list without scanning the table.
create index if not exists forge_decision_candidate_idx
    on forge_decision (created_at desc)
    where status = 'candidate';

-- ---------------------------------------------------------------------------
-- SEED v1 — the seven factory invariants already proven on PROD.
--
-- The packet is explicit: these become `active` decisions, "not more cockpit comments". They are
-- `active` from the first minute because each one has already been paid for — five of them by a lost
-- morning, and two by a gate that now fails when they stop being true. `owner` is the captain because
-- a promoted decision needs a human's name, and `evidence_sha` names what proved it (a probe, a gate,
-- an incident) rather than inventing a commit for a practice.
--
-- Idempotent: re-running the migration leaves an existing key alone, so a later edit to a statement in
-- this file cannot silently overwrite a statement that was promoted in the database.
-- ---------------------------------------------------------------------------

insert into forge_decision (key, statement, status, owner, evidence_sha, source, domain, promoted_at)
values
    (
        'batch-table-is-job-stream',
        'The ENGINE BATCH table is the job stream: if a row is in it, it goes.',
        'active', 'captain', 'probe-batch-sync', 'captain', 'forge', now()
    ),
    (
        'unattended-path-fails-closed-on-git',
        'The unattended worker refuses to start unless the repository fast-forwards cleanly to origin main.',
        'active', 'captain', 'launchd wrapper: git pull --ff-only', 'captain', 'forge', now()
    ),
    (
        'silent-refusal-is-a-defect',
        'A failure that leaves no captured record is a defect, never a preference.',
        'active', 'captain', 'instrumentation.ts onRequestError to app_error', 'captain', 'forge', now()
    ),
    (
        'abandoned-claim-is-not-running',
        'A claim whose heartbeat is older than the cleaner window is abandoned, not running.',
        'active', 'captain', 'agent work stale-claim cleaner', 'captain', 'forge', now()
    ),
    (
        'intent-is-not-status',
        'Intent is recorded in its own column or table; a status is evidence of work that happened.',
        'active', 'captain', 'board repair 2026-09-15: completion 100 with a moved status', 'captain', 'forge', now()
    ),
    (
        'withdraw-is-real',
        'Withdrawing a queued item cancels it and reports a claimed or running one instead of deleting it.',
        'active', 'captain', 'probe-engine-withdraw', 'captain', 'forge', now()
    ),
    (
        'maps-cannot-cite-dead-paths',
        'A map, manifest or citation that names a path which does not exist fails the harness gate.',
        'active', 'captain', 'pnpm forge:packet-lint rules 7, 8 and 10', 'captain', 'forge', now()
    )
on conflict (key) do nothing;
