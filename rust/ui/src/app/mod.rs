//! The application framework: the master shell, the `Screen` contract, and the screens built on it.
//! The contract and its rules: docs/agent/UI-SCREEN-ARCHITECTURE.md.
//!
//!   screen.rs    the `Screen` trait (the abstract class), `ScreenCtx`, `Link`
//!   cmd.rs       `Cmd` (side effects as data), `Endpoint`, `Remote`, `ApiError`
//!   api.rs       the API catalogue — the only place URLs are written
//!   exec.rs      the executor — the only place a `Cmd` touches the network, storage or location
//!   host.rs      `ScreenHost<S>` — runs any screen
//!   template.rs  the standard views: headings, panels, loading, failure, empty, tabs, back link
//!   list.rs      the list building block: search with a pause, paging, the list rail
//!   page.rs      the read-only page building block: `PageScreen<T>`
//!   registry.rs  every screen, in one table: routes, menus, the cutover ledger
//!   chrome.rs    the site header/footer and the portal top nav/rail, generated from the registry
//!   shell.rs     the master app: one router for the site and the portal
//!   screens/     the screens on the trait

pub mod api;
pub mod chrome;
pub mod cmd;
pub mod exec;
pub mod host;
pub mod list;
pub mod page;
pub mod registry;
pub mod rows;
pub mod screen;
pub mod screens;
pub mod shell;
pub mod template;
