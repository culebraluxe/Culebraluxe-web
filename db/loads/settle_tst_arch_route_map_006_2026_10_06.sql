-- CulebraLuxe
-- LOAD: settle_tst_arch_route_map_006_2026_10_06.sql
--
-- WHY. The row is `Failed` and the deliverable is on `main`. `TST-ARCH-ROUTE-MAP-006`'s run `35d918d4` ended
-- `Failed` on 2026-09-30 22:48 UTC *before it published anything*, so the engine never wrote a settlement and the
-- board kept the failure. The work itself was not lost: the lane committed it to `origin/lane/mimo` (`1530b011`,
-- blob `c285619c`, 475 lines) and the Captain directed its recovery on 2026-10-06. It is now on `main` as
-- `d8070947` — `tests/tests/arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical.rs`, the
-- file this story's own `scope` names as its deliverable, byte-identical to the draft the lane held — and its assay
-- passes: `cargo test -p test-harness --test arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical`
-- -> 1 passed; 0 failed, `cargo check --workspace --all-targets` ok, `rustfmt --check` clean.
--
-- So the board's `Failed` is stale, and leaving it there is not neutral: it is the fact every lane reads, and it
-- says a story whose test is on `main` was never done. This file ends that.
--
-- WHAT. Set the row `Complete` with `completion = 100`, in the engine's own `mark_story_complete` shape
-- (`db/src/forge_engine.rs`): `completed_at = coalesce(completed_at, now())`, so a timestamp already there is never
-- moved. The receipt goes on the row's `notes` the way the hold writer appends a reason, so the board carries WHY
-- the row says Complete rather than asking a reader to trust the flip. Migration 182's constraint
-- (`completion = 100` requires `status = 'Complete'`) is satisfied by the same statement.
--
-- THE FIRST STEP OF `settle_landed_candidates_2026_10_01.sql` IS DELIBERATELY ABSENT. That file cancels open
-- `agent_work_item` rows; this story has none to cancel, and the absence is measured rather than assumed: its only
-- item (`3220bed5`) is terminal `Error` since 2026-09-30 22:48, `storyboard_active_work` holds no row for it, and
-- it holds no `Staged` membership in any unfired flight. Nothing is in flight, so nothing needs taking out of it.
--
-- NOT IN THIS FILE: the other nine paths in `origin/lane/mimo`'s commit (superseded — `1d92b299` and `639dfcc8`
-- landed canonical versions of every one), the `muse`/`muse-2` drafts of this same file, and
-- `TST-ARCH-ROUTE-MAP-004` (also `Failed` with its deliverable on `main` — a separate decision, not a rider on this
-- one). Mutations are not written down in retrospective batches here.
--
-- PROD ONLY, AND THAT IS CORRECT. This row is absent from DEV (the board rows for this programme were authored
-- straight into PROD, which is by design — migration 104), so there is no DEV half to keep in step; the guard below
-- makes a DEV apply a no-op rather than a lie.
--
-- IDEMPOTENT: guarded on `status = 'Failed'`, so a re-run is a no-op and it can never overwrite a state another
-- writer has since set (`Ready`, `In Progress`, `Hold`). REVERSIBLE: set the row back to `Failed`, `completion = 0`,
-- `completed_at = null` and strip the appended note line.
--
--   cargo run --manifest-path Cargo.toml -p cli -- db-tool apply \
--     db/loads/settle_tst_arch_route_map_006_2026_10_06.sql prod \
--     --note "settle TST-ARCH-ROUTE-MAP-006: the deliverable is on main (d8070947) and its assay passes"
--
-- Applied: 2026-10-06 16:13:57 UTC -> prod ("applied … (recorded in schema_migration)", exit 0; the target line
-- read `database: target=prod host=ep-flat-art-ax92tn7a-pooler…neon.tech` before the apply was believed).
-- VERIFIED after the apply: `status = 'Complete'`, `completion = 100`, `completed_at = 2026-10-06 16:13:56 UTC`,
-- and the re-run printed `already applied … (checksum match) — skipped`, exit 0 — the idempotency above, measured
-- rather than claimed. NOT YET DONE, and deliberately: `TST-ARCH-ROUTE-MAP-004`'s identical staleness.

begin;

update storyboard_story
   set status       = 'Complete',
       completion   = 100,
       completed_at = coalesce(completed_at, now()),
       notes        = case
                        when notes is null or notes = '' then $settle$2026-10-06 settle (Captain-directed): RECOVERED — the deliverable is on main, so the row is Complete. Commit d8070947 lands tests/tests/arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical.rs (blob c285619c, 475 lines), byte-identical to the draft the lane held in origin/lane/mimo 1530b011; assay: cargo test -p test-harness --test arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical -> 1 passed, 0 failed; cargo check --workspace --all-targets ok; rustfmt clean. The run (35d918d4) failed 2026-09-30 22:48 before it published, so the work was recovered by hand, not by the engine.$settle$
                        else notes || E'\n' || $settle$2026-10-06 settle (Captain-directed): RECOVERED — the deliverable is on main, so the row is Complete. Commit d8070947 lands tests/tests/arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical.rs (blob c285619c, 475 lines), byte-identical to the draft the lane held in origin/lane/mimo 1530b011; assay: cargo test -p test-harness --test arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical -> 1 passed, 0 failed; cargo check --workspace --all-targets ok; rustfmt clean. The run (35d918d4) failed 2026-09-30 22:48 before it published, so the work was recovered by hand, not by the engine.$settle$
                      end,
       updated_at   = now()
 where id = 'TST-ARCH-ROUTE-MAP-006'
   and status = 'Failed';

-- What the board now says.
select id, status, completion, completed_at
  from storyboard_story
 where id = 'TST-ARCH-ROUTE-MAP-006';

commit;
