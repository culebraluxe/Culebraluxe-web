-- Park, not promotion: take the NINE engineering rows back OFF the queue so the board serves the one job it has.
--
-- WHY. The day's deliverable is missing TEST CODE. `restore_eng_queue_2026_09_30.sql` put the nine back on the
-- queue because a `forge clean` had swept them along with eight test rows; that restored what a sweep destroyed,
-- but it also queued nine *engine-rewrite* stories (ENG-GUARD-*, ENG-POOL-IO-01, ENG-PARITY-LEDGER-01,
-- ENG-WHATSAPP-COEXISTENCE-RUST-01, ENG-FORGE-TURN-VISIBILITY-01, TECH-FLIGHT-RECORDER-01) on a board whose whole
-- purpose today is writing tests. Measured on PROD before this apply (2026-09-30 06:0x UTC): nine open items, all
-- queued 05:12:45 — i.e. the nine would be dispatched ahead of every test row the moment the scheduler resumes.
--
-- WHAT. Two statements, one transaction, the same shape `clean` uses (rust/core/db/src/forge_reset.rs:413-461):
-- cancel the open work item(s) of exactly these nine stories, and move exactly these nine stories to `Hold`.
-- Nothing is inferred from a time window and nothing outside the nine ids is touched — this is the aimed version
-- of the sweep that caused the problem in the first place.
--
-- IDEMPOTENT: the item cancel is guarded on the open states and the story update on `Ready`/`In Progress`, so a
-- re-run against already-Hold rows is a no-op and can never cancel work that has since been claimed.
--
-- REVERSIBLE, DELIBERATELY: `db/loads/restore_eng_queue_2026_09_30.sql` is the exact inverse (it re-opens the
-- newest cancelled item and returns the story to `Ready`, which re-fires the dispatch trigger). Say the word and
-- the nine are back on the board; this file only decides what the engine works on NEXT.
--
--   ./rust/target/debug/cli db-tool apply db/loads/hold_eng_queue_2026_09_30.sql prod \
--     --note "park the nine engineering rows: the day's work is missing test code, not engine rewrites"

begin;

update agent_work_item
   set state       = 'Cancelled',
       claimed_by  = null,
       started_at  = null,
       finished_at = now(),
       error_text  = 'parked 2026-09-30: the day is test-writing; engine work waits for the captain',
       updated_at  = now()
 where story_id in (
        'ENG-POOL-IO-01',
        'ENG-WHATSAPP-COEXISTENCE-RUST-01',
        'TECH-FLIGHT-RECORDER-01',
        'ENG-FORGE-TURN-VISIBILITY-01',
        'ENG-GUARD-ENV-RUST-01',
        'ENG-GUARD-AGENTS-LINT-01',
        'ENG-GUARD-REPO-RUST-01',
        'ENG-GUARD-FORGE-RUST-01',
        'ENG-PARITY-LEDGER-01')
   and state in ('Ready', 'Claimed', 'Running', 'Paused');

update storyboard_story
   set status = 'Hold',
       updated_at = now()
 where id in (
        'ENG-POOL-IO-01',
        'ENG-WHATSAPP-COEXISTENCE-RUST-01',
        'TECH-FLIGHT-RECORDER-01',
        'ENG-FORGE-TURN-VISIBILITY-01',
        'ENG-GUARD-ENV-RUST-01',
        'ENG-GUARD-AGENTS-LINT-01',
        'ENG-GUARD-REPO-RUST-01',
        'ENG-GUARD-FORGE-RUST-01',
        'ENG-PARITY-LEDGER-01')
   and status in ('Ready', 'In Progress');

commit;
