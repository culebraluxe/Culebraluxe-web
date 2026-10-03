# TECH-FLIGHT-RECORDER-01 — the Flight Recorder console, ported to Rust/Yew

Lane: builder. Research done 2026-09-28 (the owner designed this console; port it faithfully — its structure is the
spec, not a suggestion). Status: **implemented and live-verified (2026-09-29)** — see "Verification" at the end.

## Goal (one line)

`/portal/tech/flight-recorder/:instanceId` shows a Forge/workflow run end to end again — the owner's five-view
console — reading the Rust trace that already exists, with nothing in JavaScript.

## Where it plugs in

Forge (the engine that builds stories off the storyboard) records every run's trace events; the TECH Cockpit links a
run to this console. Without it a run is a black box. It matters once stories run again (the storyboard is being
cleaned first).

## What exists (verified)

| Piece | State |
| --- | --- |
| The trace read | **Rust, done.** `db/src/flight_recorder.rs` (846 lines), `middle/model/src/flight_recorder.rs` (`FlightRecorderTransaction`: transaction context, workflows with `graph` + `node_states`, events, `instances` window), `web/src/flight_recorder/mod.rs` (`transaction(instance_id)`, authorized + audited). |
| A route | `/v1/flight-recorder/{id}` only (`routes.rs`) — **the internal API; a page cannot reach it.** |
| The screen | `web/ui/src/app/screens/flight_recorder.rs` — a "widget removed" placeholder. The list (`tech-flight-recorder`) is `Nav::Retired`; `portal_bridge.rs` still answers `screen=tech-flight-recorder` (with `workflows`). |

## The original (read in full — `git show 4cf98110^:<path>`)

- `components/portal/tech/flight-recorder-console/FlightRecorderPage.tsx` (1,288 lines). Dark console
  (`bg-[#0b1220]`). **Header**: "FR" mark, title + "End-to-end transaction timeline & causality", search ("Search
  events, entities, correlations…"), Auto Refresh / Live chips. **Left rail (280px)**: correlation id; root title +
  kind chip; four stats Duration / Events / Systems / Status; the amber "Showing the newest N of M workflow instances"
  line when capped; Business Context (Deal, Property, Client, Workflow, Initiated By, At — only present ones); Master
  Workflow mini-map; kind filter list with counts (toggle). **Tabs**: Timeline · Workflow Graph · Causality Graph ·
  System Swimlane · Raw Events; plus Filter (count when filtered, clears), density Compact/Expanded (row 56/88px),
  Export (whole trace JSON), Download (filtered events JSON). **Right panel (360px)**: kind glyph + title + status;
  Overview (Event ID, Time, Duration, System, Correlation ID, Causation ID — clickable to the cause, Event Type);
  Payload (pretty JSON + Copy); Tags (click = search); Related Events (click = select, signed offset).
  **Cross-selection**: selecting an event selects its workflow node (`workflowNodeId`); selecting a node highlights its
  events (amber row + left border). Arrow Up/Down move the selection, Escape clears.
- Timeline (virtualized in React): columns Time (clock + offset) · Event (kind glyph tile, title, subtitle) · Details
  (first 2 entries compact, all expanded) · System chip.
- Workflow Graph = the mini-map full-pane at 1.9×. **Layered layout** (`layoutMasterWorkflow`): layers by BFS from
  nodes with no incoming transition (else the first node), unvisited nodes appended as their own layers; x = 30 +
  layer×170, y = 30 + row×58, radius 10. Node fill = semantic kind colour; execution state is an overlay only
  (NOT_VISITED dimmed 0.35/0.4, CURRENT gold ring 2.5, FAILED red ring/stroke, RECOVERED amber ring, COMPLETED ✓;
  selected white ring). Title tooltip "name — state (kind)".
- Causality Graph: `buildCausalGraph(toTimelineEntries(events))` + `layoutGraph` from `lib/causal-graph.ts`; selected
  node + its parents/children highlighted (gold), others dimmed 0.4; "? missing cause" under nodes with unresolved
  causation; arrows via SVG markers. Empty: "No causal relationships recorded for these events." (Forge events carry no
  causation id, so this is legitimately empty for engine runs — the Workflow Graph is the one with content.)
- System Swimlane: one lane per system (`groupEventsBySystem`), time → with a bounded scale `x = pad+label+sqrt(ms/max)
  ×span` (ruler ticks at 0/25/50/75/100%), cross-system causal hand-offs dashed gold, markers = kind glyph in a ring;
  1100-wide viewBox, `min-w-[900px]`.
- Raw Events: table Time · Event type · Event id · System · Status · Node · (+/−); expanded row shows
  `rawEventFields(event)` in two columns and the payload with Copy JSON.
- Semantic colours (shared): Command #a78bfa, DomainEvent #60a5fa, Workflow #34d399, Task #c6a15b, Integration
  #f472b6, Persistence #22d3ee, Unknown #94a3b8.

## The adapter (read in full — `lib/flight-recorder-adapter.ts`, path `adaptFlightRecorderTransaction`)

- `eventTypeToKind`: prefixes COMMAND_/COMMAND. → Command; DOMAIN_EVENT → DomainEvent; WORKFLOW_ NODE_ TRANSITION_
  PROCESS. TOKEN. → Workflow; TASK_ TIMER_ JOB_ (and dotted) → Task; SIGNATURE_ → Integration; DOCUMENT_ PERSISTENCE_
  → Persistence; else Unknown.
- `systemToSystemId(source_system)`: contains domain → Domain Model; command/api/gateway → API Gateway; workflow →
  Workflow Engine; task → Task Service; boldsign/signature → BoldSign; postgres/sql/persist → PostgreSQL;
  forge/observer → **Forge Observer**; else Unknown (raw kept in details as "Raw System").
- `outcomeToStatus(outcome, eventType)`: the explicit outcome first (FAILURE/FAILED/ERROR → Failed; STARTED/PENDING →
  Pending; SUCCESS/COMPLETED/RECOVERED/REPLAYED → Success), then the event type's suffixes; absence of failure is not
  success (→ Unknown).
- `nodeTypeToKind(node type)`: command; state/domain/domain_event; task/user_task/human; integration/external/
  provider/signature; persistence/document/storage; start/end/decision/fork/join/timer/subprocess/workflow; else Unknown.
- Event: details (Summary; Node = mapped name, Node ID; Command; Domain Event; Document; Signature; Raw System; then
  metadata entries until 10), title = summary or humanized type, subtitle = kind label, offset = ms since the earliest
  event, tags = {type, source system, status} lowercased, `relatedEventIds` empty in this path.
- Summary: correlation id (transaction's, else the primary workflow instance id); root = first event; duration = max
  offset; status Failed if any failed, InProgress if the transaction is `active`, else Completed; business context
  from the transaction; `instances` carried through.
- Workflow view: nodes from `graph.nodes` (name, type, semantic kind, state from `node_states`, default NOT_VISITED);
  transitions from each node's `transitions`.

## Still to read before building

`lib/flight-recorder-views.ts` (135 lines: `buildUnresolvedCauses`, `buildCausalEventPairs`, `buildSelectionCausality`,
`groupEventsBySystem`, `isSelectedCausalEdge`, `rawEventFields`), `lib/causal-graph.ts` (`buildCausalGraph`,
`layoutGraph`), `components/portal/tech/flight-recorder-console/useFlightRecorderState.ts` (filters, density, the
default selection), `format.ts` (`formatClock`, `formatDisplayTime`, `formatDuration`, `formatOffset`), `KindGlyph.tsx`
(the per-kind glyphs — port the paths into `web/ui/src/icons.rs` style).

## Build plan

1. `GET /api/portal/flight-recorder/{instanceId}` in `portal_bridge.rs`, calling `services().flight_recorder()
   .transaction(..)` (same auth as the other portal reads).
2. Pure Rust in `web/ui/src/flight_recorder.rs`: the adapter, the view helpers, both layouts, the formats — with unit
   tests for each classifier and both layouts (fixtures from a real Forge trace on DEV).
3. The screen on the `Screen` trait (`web/ui/src/app/screens/flight_recorder.rs`): the three columns and five views
   above, SVG for the graphs and swimlane, keyboard on the timeline container (not a window listener). A trace of a few
   hundred events renders without virtualization; window it only if a real trace needs it.
4. Put the list back in the Tech menu (`model.rs` `tech-flight-recorder` `Nav::Retired` → listed) and check the Cockpit
   links land. Test in WebKit (the owner uses Safari) against a real run on DEV.

## Non-goals

No change to the trace read or its SQL; no new trace data; no JavaScript (no virtualizer library, no graph library);
no Forge changes.

## Verification (2026-09-29)

Landed by `dcf2be3f` (the port: `web/ui/src/flight_recorder.rs`, the screen, the portal route
`/api/portal/flight-recorder/{id}`) and `2110d6a5` (timeline arrow `preventDefault`). `dcf2be3f` is on `origin/main`;
`2110d6a5` is local only — this node's brief says **do not push**, so nothing was pushed from here.

- **Classifiers and both layouts, as unit tests:** `cargo test -p ui flight_recorder` → **18 passed, 0 failed**
  (event-type/system/outcome/node-type classifiers and formats, the adapter, `layout_master_workflow`,
  `layout_causal_graph`, the view projections, and five screen-reducer tests).
- **The wasm artifact:** `pnpm ui:check` clean (`cargo check -p ui --features wasm --target wasm32-unknown-unknown`).
- **The route against real DEV:** the Rust API was run with `APP_ENV=development` (boot line
  `database_target=dev`) and, with the dev stub, `/api/portal/flight-recorder/{id}` returned three real Forge traces
  (25/16/16 events; each a 71-node workflow with 176 transitions and 71 node states; system `forge_observer`).
- **All five views, in WebKit (Safari's engine):** the built app (wasm 09:43, matching HEAD) was driven with
  Playwright `webkit` at `/portal/tech/flight-recorder/{id}`. Timeline showed real `RUN_START`/`RUN_END` rows; the
  Workflow Graph showed the real node names (Architect, Architect Review Gate, Research Complete, Deploy, …); the
  Causality Graph, System Swimlane (Forge Observer lane) and Raw Events table all rendered. **No page errors, no
  "could not load" banner.** The literal Safari confirmation on the owner's machine remains his manual step.
- **No TypeScript:** the commits touched only `rust/**/*.rs` and `web/ui/Cargo.toml`.

**Deviation from build plan step 4 — the old list is deliberately NOT put back in the Tech menu.** There is no
registry mount for `tech-flight-recorder`, its retirement is a deliberate owner decision recorded in `model.rs`, and
the registry test `the_rail_is_the_designed_menu_in_its_order` fixes the Tech rail to *Cockpit / Story Board / UI Lab*.
The console is reached from the Cockpit (`web/ui/src/app/screens/tech/view/engine.rs:105` and `workbench.rs:177`) and
by direct URL (`trace-record`, `web/ui/src/app/registry.rs:199`). Listing the old list would be a dead link and would
reopen a decision the registry closed.
