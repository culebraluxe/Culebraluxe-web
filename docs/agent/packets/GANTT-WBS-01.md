# GANTT-WBS-01 — canonical schedule foundation

This packet tracks the requested WBS 1.1–3.1. Status is changed to DONE only after the story's code or evidence and targeted Rust verification are committed. Production deployment is outside this packet.

| ID | Status | Evidence / completion criterion |
| --- | --- | --- |
| 1.1 Legacy parity fixtures | DONE | The original `project-timeline.tsx` was read-only SVAR with Work item / Start / Days sortable columns, hierarchy, status progress, notes and dark skin. `timeline-projection.ts` assigned a three-day bar to every item, sample dates to undated items and synthetic finish-to-start links only when *all* items were undated. Native fixtures below distinguish those samples from stored facts. |
| 1.2 Date semantics | DONE | This packet defines the canonical rules below; Rust domain test cases are committed. |
| 2.1 Nullable WBS dates | IMPLEMENTED — verification pending | Migration 226 was applied and recorded on DEV; SQL date round trip passed inside a rolled-back savepoint. Rust build still required before DONE. |
| 2.2 WBS service persistence | IMPLEMENTED — verification pending | Authorized create/save, portal read and date validation. Rust tests still required before DONE. |
| 2.3 Dependencies | IMPLEMENTED — verification pending | Migration 227 was applied and recorded on DEV; a project-scoped link inserted and read inside a rolled-back savepoint. Rust tests still required before DONE. |
| 3.1 Pure timeline projection | IMPLEMENTED — verification pending | Planned spans, deadline, unscheduled and summary facts, explicit link geometry and Rust fixtures. Rust tests still required before DONE. |

## Parity fixture matrix

| Fixture | Stored WBS facts | Required projection |
| --- | --- | --- |
| Empty | No project items | Empty timeline with a server-supplied Today date. |
| Undated | Leaf without dates | Unscheduled; no bar and no link. Legacy showed a sample three-day bar and sample link. |
| Deadline only | Leaf with `due_at` | Milestone on the UTC calendar date; no task duration. Legacy showed a three-day bar ending there. |
| Planned | Leaf with start and finish | Inclusive scheduled bar; a separate deadline milestone only when `due_at` exists. |
| Mixed | Dated and undated siblings | Only dated facts are plotted; all rows remain visible. |
| Nested | Parent with dated/undated descendants | Parent summary spans only real descendant schedule dates; deadline envelope remains separately identifiable. |
| Dependent | Two items with a persisted finish-to-start edge | Link by stable WBS IDs, even if one item is unscheduled; no manufactured dates. |

## Canonical scheduling rules

1. `planned_start` and `planned_finish` are nullable SQL `date` values and JSON `YYYY-MM-DD` dates. They describe a task's intended work span; `due_at` remains a separate timestamp deadline. An existing item is not backfilled from the legacy sample schedule.
2. A plotted task bar requires **both** planned dates. One missing endpoint is a partially scheduled item, not a one-day task. Finish is inclusive: same-day duration is one day; September 10–12 is three days.
3. Start must not be after finish. The service rejects malformed dates or an inverted pair. It does not move `due_at` or enforce a relationship between deadline and planned finish.
4. Due-date calendar display reads the first ten ISO date characters, preserving the stored UTC day in Puerto Rico. Planned dates are calendar dates without timezone conversion.
5. Summary spans are projections over descendants; they never write dates back to parent rows. A parent may also have its own explicitly planned span, which is identifiable as canonical.
6. Dependencies are explicit WBS relationships, never inferred from row order. First supported relation is finish-to-start. They do not automatically move dates in this slice.

## Boundaries

WBS domain owns scheduling validation. WbsDao owns bound SQL. WbsService owns authorization, audit and commands through the existing Abstract service runtime. Portal bridge transports the canonical read model. Yew `Model`/`Msg`/`update` own UI state; `view` renders a pure Rust timeline projection. No TypeScript implementation or direct UI-to-database path.

## Verification record

- DEV branch `br-solitary-star-axgusezm`: both migrations are in `schema_migration` with matching SHA-256 checksums. The new `date` columns and dependency table were verified through a transaction savepoint; the probe was rolled back, leaving zero planned items and zero links.
- `git diff --check` passed. `cargo test` could not start: this execution environment has neither `cargo` nor `rustc`, and the Rust distribution host timed out. No Rust test result or compile claim is made.
- The commits are local on `main`; no push or production schema change has occurred. The Yew renderer remains the existing deadline-milestone view. WBS 3.2 is the separate bar-rendering story.
