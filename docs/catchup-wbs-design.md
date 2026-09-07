# Catch-Up → `work` (WBS) — greenfield design note

Status: greenfield (the old Catch-Up screen was never really used; its calendar is the
only thing that worked well and must be kept). This note is the durable source of truth
so the model survives session resets.

## Goal
A lightweight project-management system whose **key output is follow-up**: "what needs
attention today." The **Catch-Up screen is the operator's daily queue** over that system.
It must reuse the Portal design language and keep a working calendar.

## Domain model (deliberately small)
- **Project** — a purposeful bundle ("Onboard client X + property Y", "List property Z",
  "Q3 marketing") with owner/status/dates and optional entity links
  (person / property / contract / deal).
- **WorkItem = the unit of work (WBS node)** — belongs to a Project (optional parent for
  real WBS trees) OR is a standalone **ad-hoc follow-up** (`projectId = null`). Carries:
  - `category` → a fixed dimension mapped to the major functional areas:
    **Clients · Contracts · Property · Media · Marketing · Accounting**
  - status (open / doing / done / dismissed), due, owner, order, notes
  - optional entity link so a follow-up always knows what it belongs to.
- **Category** — a *dimension* for grouping/boards, NOT a managed entity.
- **Playbook / template** (later) — a stored project skeleton that instantiates a
  Project + its WorkItems ("onboard client+property → intake, pics scheduled, listing
  drafted…"). Out of the first build.

## Screen concept (Catch-Up = daily queue)
- Group the open/due work items by category/area and by entity (person/property).
- Operator "today" view: what is due / needs response / is blocked.
- **Keep the working calendar** (real showings + evaluation events) as a pane.
- Follow the existing Portal visual system (Panel / glass, Command+Status band, navy/gold).

## New architecture (matches Clients/Forms we already did)
1. **`services/wbs` domain** (repository port + in-memory + sql adapters):
   - queries: `queue.due`, `wbs.tree`, `wbs.get`, `project.get`
   - commands: `wbs.create`, `wbs.save`, `wbs.complete`, `project.create`
   - authorizes via the enforced resolver; returns ServiceResult; normalizes at repo.
2. **`ui/wbs-lens`** MVI controller (mirrors client-lens) + `InMemory`/`Http` sources.
3. Server page becomes a thin loader; client View binds the controller; legacy component
   stays as reference; rollback = git revert.

## DB strategy
- Reuse the existing task/calendar rows where sane.
- Prefer **minimal schema**: likely keep task→`work_item`, add `project` + a `category`
  dimension; add a migration only when the reuse decision is confirmed on the real schema.
- Out of scope for the first build: playbook instantiation, the CRM/entity activity feed,
  any destructive schema change.

## Success bar
- A `work` service with a few operations is silo-testable (DB-free).
- Catch-Up screen shows the daily queue grouped by category, with the calendar, in the
  Portal design language, built on `services/wbs` + an MVI controller.
