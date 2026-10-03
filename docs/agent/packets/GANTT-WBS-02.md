# GANTT-WBS-02 — native timeline interaction batch

Scope: WBS 3.2–5.1 from the legacy SVAR parity plan. All new UI, state, projection and tests are Rust/Yew MVI. The existing WBS service owns schedule writes and dependency validation; the portal bridge transports its answer.

| ID | Status | Implementation and acceptance evidence |
| --- | --- | --- |
| 3.2 Bars and columns | IMPLEMENTED — Rust verification pending | Planned bars require both stored dates, inclusive duration in Days, separate due milestone, unscheduled leaves. Project and descendant spans are visually distinct. Renderer `web/ui/src/app/screens/projects/view/timeline.rs`; projection `web/ui/src/timeline.rs`. |
| 3.3 Grid sorting | IMPLEMENTED — Rust verification pending | Title, Start and Days sort only siblings; default remains WBS order. Pure Rust sibling test and reducer test. |
| 3.4 Dependency connectors | IMPLEMENTED — Rust verification pending | Explicit finish-to-start edges by WBS IDs; geometry only when both endpoints have planned spans and visible rows. No synthetic links. |
| 4.1 Planned date editing | IMPLEMENTED — Rust verification pending | Date fields in selected work editor, MVI validation and WBS save; planned bar drag preserves duration and rolls back on API failure. Rust reducer test checks command and rollback. |
| 4.2 Dependency editing | IMPLEMENTED — Rust verification pending | Select predecessor, add/remove in selected work pane; portal bridge calls WbsService with canonical context and returns refreshed page. Service validates project membership and cycles. Rust reducer test checks command shape and failure. |
| 4.3 Navigation and access | IMPLEMENTED — Rust verification pending | Today, date jump and previous/next scale navigation; separate native expand/select controls; native date inputs and buttons provide a keyboard path for scheduling. |
| 5.1 Visual/progress parity | IMPLEMENTED — QA pending | Navy/gold timeline, distinct solid planned bars, outline descendant spans and diamond deadlines. Only actual WBS status completion contributes to project percent; no invented 60% task progress. User visual QA remains. |

## Verification

`git diff --check` passed. Rust tests were added to `web/ui/src/timeline.rs` and `web/ui/src/app/screens/projects/mod.rs`; this workspace has no `cargo`, `rustc` or `rustfmt`, so they have **not run**. A host and wasm build, targeted Rust tests and the user's visual QA are outstanding. Do not mark a row DONE until its verification is recorded. No production deployment or schema mutation is in this batch.
