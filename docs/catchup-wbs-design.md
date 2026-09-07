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

## Proposed schema (first cut)
Decision point: reuse the legacy `public.task` vs. new tables. Recommendation: new `project` +
`work_item` tables (clean, no legacy baggage), leave `public.task` alone for now.

```sql
-- lightweight project root (WBS top)
create table if not exists wbs_project (
  id text primary key,
  name text not null,
  owner text,            -- app_user id or free text owner
  status text not null default 'open',
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

-- the unit of follow-up work (a WBS node OR standalone ad-hoc item)
create table if not exists wbs_item (
  id text primary key,
  project_id text references wbs_project(id) on delete cascade,  -- null = ad-hoc follow-up
  parent_id text references wbs_item(id) on delete set null,     -- real WBS nesting
  title text not null,
  notes text not null default '',
  category text not null,     -- clients|contracts|properties|media|marketing|accounting|management
  status text not null default 'open',  -- open|doing|done|dismissed
  due_at timestamptz,
  owner text,
  sort_order int,
  entity_type text,           -- person|property|contract|deal
  entity_id text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
```

- `category` is validated at the service boundary against the WBS_CATEGORIES catalog.
- All driver-typed columns are normalized to application shapes at the SqlWbsRepository
  boundary (dates → ISO/null, ints → number/null) per the AGENTS normalization rule.
- First migration number + DEV/PROD apply happen when the SqlWbsRepository is built.


## Success bar
- A `work` service with a few operations is silo-testable (DB-free).
- Catch-Up screen shows the daily queue grouped by category, with the calendar, in the
  Portal design language, built on `services/wbs` + an MVI controller.
