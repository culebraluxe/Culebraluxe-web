# Layer: FORGE (separate on purpose)

Crate: **`rust/forge`**. **Owns:** the engine that *builds this product* — not the product. If you are changing how a
client's transaction behaves, you are in [WORKFLOW.md](WORKFLOW.md), not here. Forge is tooling, and nothing the product
serves depends on it.

## The boundary

| | |
| --- | --- |
| **In Forge** | work items, roles, story packets, gates, evidence, decisions, deployment records, the SDLC workflow |
| **Not in Forge** | anything a customer sees, or any rule about a deal, contract, client or property |
| **Shares with the product** | the one database pool (`db::shared`) and the `app_error` sink — which is why a Forge failure is visible in the same place as a product failure |

## Layout

`rust/forge/src/engine/` — around sixty modules. Groups, not an index:

| group | examples | what they are |
| --- | --- | --- |
| **RE transaction runtime** | `re_runtime`, `re_commands`, `re_facts`, `re_receipt`, `re_port` | the *product* path the API calls. See WORKFLOW.md. |
| **Delivery workflow** | `dispatch`, `executor`, `commands`, `completion`, `claim_blocker`, `recovery` | moving a work item through its states |
| **Roles and agents** | `agents`, `architect`, `smith_candidate`, `scout…` | which role does what, and with which model |
| **Gates** | `evidence_gate`, `evidence_store`, `assay`, `baseline` | proving work before it is accepted |
| **Records** | `db_writer`, `packet`, `decisions`, `closure` | what gets written down, and where |
| **Deployment** | `deploy`, `deploy_intent` | releasing |
| **Vendor/tooling** | `vendor_session`, `opencode`, `worktree`, `alerts` | external process and session handling |

`src/engine/mod.rs` is the module index; start there.

## Control plane — read this before running anything

**`pnpm forge:clean` sets `APP_ENV=production` and runs with `--force`.** It resolves to the **production database**.
That is by design (the Forge control plane lives there), and it means it is **not** local hygiene: it needs the
Captain's go, like any production action. `AGENTS.md` carries the same warning.

Check the target before believing any command is on dev: the API's boot line, `GET /v1/diagnostics/db`, or
`pnpm db:migrations` (per-target state).

## Where the deep documentation is

`docs/agent/WORKFLOW-ARCHITECTURE.md` — 590 lines on Forge: roles, work items, the snapshot contract, the publish path,
budgets, and measured facts. That is the old *long* style of doc, and it is genuinely good for this subject; read it when
you work on Forge rather than expecting this page to cover it.

Also: `docs/agent/FORGE-WORKSHOP.md`, `docs/agent/packets/<STORY-ID>.md` (per-story work), `docs/agent/skills/`.

## Rules that apply here like anywhere else

Binds not string SQL (some legacy `psql_query` calls remain — convert the file you are in, do not add more); failures
surface as `DbFailure`/`ApiError` so they land in `app_error`; and the repository and the database are the authority, not
a packet or a previous run's report.
