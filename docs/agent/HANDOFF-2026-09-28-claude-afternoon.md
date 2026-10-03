# Handoff — 2026-09-28 afternoon (Claude, worktree `Culebraluxe-web-claude2`)

Read `AGENTS.md`, `docs/agent/ORIENTATION.md`, `docs/agent/MEMORY.md` (top entries are today's), then this.

## 1. STATUS — what is true right now

| Fact | Proof |
| --- | --- |
| PROD schema = DEV schema | `pnpm db:parity` → PARITY OK (after 217, 219, 223, 250 went to PROD today) |
| Lead emails work in PROD; notice subject now starts `LEAD:` (ships next deploy) | test lead 12a3880a saved + emailed 19:10Z; commit 50222f5b |
| Card-size listing photos live in PROD; ferry photo is a real JPEG | MEDIA-CARD-PROD-01 Complete; `/api/media/61822357…?size=card` 200 |
| The old global UI loop is deleted; 56 of 58 registry entries are `Screen`s | c0b9b1a3; `web/ui/src/app/registry.rs` |
| Catch-Up lives in Projects (Today / Unscheduled / People); off the CORE nav | 4ef9b58d |
| Projects navigator redesigned with the owner (denser rows, pole click, lens flip keeps project, Seen from, keys, due dates, overdue counts + bell to hide them, Books) | a1526ec9, d2f5b6c9 |
| Grok review items 1–8 fixed; the `operation` alias removed | c588bd7c … e5855957, cb4518f4 |
| `cargo test -p ui` (plain) runs 128 tests; `pnpm ui:check` clean | `wasm` is a default feature of `ui` |

## 2. HOLDS

| Held | Owner | Do |
| --- | --- | --- |
| DEEP1 storyboard row (data pipeline doctrine) | owner / DeepSeek (Apple contacts work) | do not edit until the owner says |
| Forge (`arch_ran` never set, dead forge:* commands, split door) | DeepSeek | leave |
| Storyboard state | owner | PROD only — never write stories to DEV |
| PROJECTS-TREE-01 | owner | close only after his Safari sign-off |

## 3. WHERE TO LOOK

| Task | Place |
| --- | --- |
| Projects screen | `web/ui/src/app/screens/projects/` (`mod.rs`, `selection.rs`, `edits.rs`, `nav.rs`, `catch_up.rs`, `view/*.rs`, `tests.rs`) |
| URL catalogue | `web/ui/src/app/api.rs` (`Endpoint`, `FileEndpoint`, `api::auth`) |
| Service door rule | `docs/layers/SERVICES.md` "Two doors, one implementation" |
| ARCH-HANDOFF row | generated from `docs/agent/ARCH-HANDOFF.md` by `db/loads/arch_handoff_sync.sql` (`db-tool apply … --force` after regenerating) |

## 4. DONE — see STATUS; every commit is on `origin/main`.

## 5. NOT VERIFIED

- Nothing from today is deployed except what the owner deployed at ~19:00 (lead emails worked). Everything after
  50222f5b ships with the next `pnpm deploy:prod`.
- The Projects navigator changes were verified in WebKit against DEV, not in the owner's Safari.

## 6. OPEN — in order

1. DONE: `mailbox.rs` split (637 lines + `mailbox/tests.rs`). DONE-BY-COMPILER: a service on `abstract_service!` cannot
   grow an envelope operation — adding its own `dispatch` is a second `AbstractService` impl, `E0119`.
3. DONE: Grok's vertical review of clients (screen → `api::ClientsRead` → `portal_bridge`/`routes.rs` → `ClientService`
   → `ClientDao`). One door: the routes call the service; every method authorizes then audits; the person-admin write and
   its cache refresh live once (`apply_person_admin_update`, shared by `/v1` and the portal); the DAO binds every value.
   One naming wrinkle left on purpose: the service authorizes and audits under domain `"clients"` while its registry
   descriptor says `"client"` — aligning it would split the audit history under two names, so it needs the owner's word.
4. Screens over 800 lines split on their next edit (workbench/view.rs 2,492; deals/view.rs 1,642; forms.rs 1,706;
   system_health.rs 1,351; projects/view/calendar.rs 940).

## 7. ASK THE OWNER

Nothing blocking. Deploy when ready (`pnpm deploy:prod`) to ship today's UI work.

## 8. EVENING — Grok's 100-list, done (proof: `docs/agent/PROOF-2026-09-28-grok-100.md`)

| Bar | What landed | Commit |
| --- | --- | --- |
| MVI contract | no URL literal and no `web_sys` in `app/screens` (links in `api::links`/`api::auth`, browser reads in `app/exec`); every load state from `template::remote`/`remote_toned`/`loading_line` | `ae2f3b93` |
| Service doors | no HTTP handler touches a DAO (diagnostics via `SupportDiagnosticsService::db_counts` + one explicit System grant; app events via the capture seam); `clients`/`client` audit name recorded as decided | `0886b229` |
| Docs match HEAD | `AGENTS.md` Rust First and `rust/README.md` describe the one Rust app | `1aa4170f` |
| Shared policy | the root-only codes have one source (`model::security`), bound into the SQL guard; one test per reader | `a2c4c9bb` |
| Boundedness | every `.rs` file under `rust/` is at most 800 lines except DeepSeek's four live files (`cli/src/forge/lint.rs`, `core/domain/src/apple_messages.rs`, `applemail.rs`, `cli/src/apple_mail.rs`) — split them on their next edit | `360daa13` … `41ca7658` |

How the splits were made (move only): contiguous top-level ranges moved to child modules with `use super::*`; moved items
widened to `pub(super)` (or kept `pub` with `pub use` re-exports where the old path is public); an oversized inherent `impl`
continued as several `impl` blocks; trait impls never split; `macro_rules!` left in the parent ahead of the modules.

