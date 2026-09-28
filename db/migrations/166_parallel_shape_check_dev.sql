-- 166: DEV learns the parallel-shape rule PROD has enforced all along.
--
-- PROD carries `agent_work_item_parallel_shape_check`; DEV never did, and until
-- this weekend nobody could see that, because `pnpm db:parity` compared
-- tables/columns/indexes/FKs and never read check constraints (fixed in
-- FORGE-PARITY-CHECK-01, which is what surfaced this).
--
-- What the constraint ACTUALLY bites, verified 2026-09-12 rather than assumed — and
-- this correction matters, because the constraint looks stronger than it is. A CHECK
-- fails only when it evaluates to FALSE; NULL PASSES. Its second branch tests
-- `lane = 'smith'` and `parallel_size between 2 and 3`, and no code path writes
-- `lane` or `parallel_size` at all (no defaults, no triggers, no writer anywhere in
-- the repo). So for a real grouped row those tests are NULL, the whole predicate is
-- NULL, and the row is admitted. PROD proves it: 17 grouped rows with `lane IS NULL`
-- and `parallel_size IS NULL` sit there happily. The clause that does bite is the
-- first one, on a row where `parallel_group_id IS NULL` while `parallel_slot` is
-- present (that makes `parallel_slot IS NULL` a definite FALSE rather than NULL).
--
-- That is exactly the shape two DEV rows carried — `parallel_slot = 0` (and one
-- with a slot but no group), `split_assignment = 'a'`, created 2026-09-10
-- 07:03-07:04, from the split-lane probing. PROD would have refused them; DEV
-- accepted them, so the "passes in DEV, fails in PROD" direction was real, just
-- narrower than it first looked. Their writer was `recordSplitChildAssignment`,
-- which set `split_assignment` and `parallel_slot` without ever setting the group;
-- it now writes the group from the same source the enqueue uses, and both the
-- enqueue and that writer now refuse a half-grouped shape by name. The two rows are
-- removed in the same weekend pass: ADD CONSTRAINT validates existing rows, and
-- these two are the only rows in DEV that evaluate to FALSE, so it would (correctly)
-- refuse to install while they sat there. Relaxing to NOT VALID was rejected — it
-- would have hidden the drift this migration exists to close.
--
-- Known weakness, reported and deliberately NOT changed here: because `lane` and
-- `parallel_size` are never written, two clauses of this constraint are inert. Making
-- them real (`parallel_size IS NOT NULL`, `lane IS NOT NULL` for grouped rows) would
-- invalidate PROD's 17 existing rows and would break the split lane until something
-- actually writes those columns. That is a schema-and-writer change with data
-- consequences — a captain decision, not a side effect of closing parity.
--
-- Idempotent: the guard means a database that already has the constraint (PROD) is
-- left untouched, so this is safe to apply to both — and it must be applied to both,
-- because the constraint reached PROD by hand and had no migration until now.
--
-- Applied: DEV 2026-09-12 (the repair matched 2 rows, then the constraint installed),
-- PROD 2026-09-12 (repair matched 0 rows; constraint already present, so the DO block
-- is a no-op — recorded in the ledger so "what was run where" stays answerable).

-- 1. Repair the half-grouped shape, which is meaningless data as well as a violation:
--    a parallel slot (and a parallel size) belongs to a group, so a row that has one
--    with NO group is not a serial row with extra detail, it is a malformed one. Two
--    rows matched — both ENG-FORGE-SPLIT-DOGFOOD-01, state Done, `parallel_slot = 0`,
--    `split_assignment = 'a'`, from the 2026-09-10 split-lane probing. They are
--    REPAIRED rather than deleted on purpose: one is referenced by
--    `forge_engine_task_execution.work_item_id`, and its run history is worth more
--    than the tidiness of a DELETE. `split_assignment` is left alone as history (it
--    is only read for a real group), and clearing the two columns is what makes the
--    row evaluate TRUE instead of FALSE.
--
--    Idempotent: after the first run no row has a slot without a group, so this
--    matches nothing.
update agent_work_item
   set parallel_slot = null,
       parallel_size = null,
       updated_at = now()
 where parallel_group_id is null
   and (parallel_slot is not null or parallel_size is not null);

-- 2. The constraint itself, "safe if already present" without procedural SQL.
--
--    DROP IF EXISTS + ADD rather than a `do $$ ... $$` guard: the migration runner
--    sends the file as ONE simple query through the WebSocket pool, and `$$` is read
--    as a lexer token (scanner_yyerror) on that path. Drop-then-add is idempotent in
--    the way that matters here — the statement set can be re-run any number of times
--    and always ends with exactly this constraint defined — and on PROD it is
--    verified to be a no-op in effect: the definition was captured before and after
--    the apply and is byte-identical (pg_get_constraintdef), so no rule was silently
--    replaced. PROD's 17 existing grouped rows pass because they evaluate NULL (see
--    above), not because anything was relaxed.

alter table public.agent_work_item
  drop constraint if exists agent_work_item_parallel_shape_check;

alter table public.agent_work_item
  add constraint agent_work_item_parallel_shape_check
  check (
    (parallel_group_id is null and parallel_slot is null and parallel_size is null)
    or
    (
      parallel_group_id is not null
      and lane = 'smith'
      and parallel_slot between 1 and 3
      and parallel_size between 2 and 3
      and parallel_slot <= parallel_size
      and split_assignment is not null
      and btrim(split_assignment) <> ''
    )
  );
