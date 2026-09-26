//! The SUPPORT screens that are a table of facts: roles, authorities, the Mux video test and the token review page.
//! Each is a `RowsSpec` (app/rows.rs).

use crate::app::api::RowsRead;
use crate::app::rows::RowsSpec;

/// `/portal/settings/roles`.
pub struct Roles;

impl RowsSpec for Roles {
    const EYEBROW: &'static str = "Security";
    const TITLE: &'static str = "Roles";
    const PURPOSE: &'static str = "Every security role, whether it is for internal staff or external guests, and how many entitlements it holds.";
    const COLUMNS: &'static [&'static str] = &["Role", "Account", "Entitlements"];
    const EMPTY: &'static str = "No roles are defined.";
    fn read() -> RowsRead {
        RowsRead::portal("settings-roles")
    }
}

/// `/portal/settings/authorities`. KNOWN GAP, carried from the old wiring rather than hidden: its rows are the same
/// role-entitlement read as Roles, not a list of authorities. Recorded in docs/agent/UI-SCREEN-ARCHITECTURE.md.
pub struct Authorities;

impl RowsSpec for Authorities {
    const EYEBROW: &'static str = "Security";
    const TITLE: &'static str = "Authorities";
    const PURPOSE: &'static str =
        "What each role is permitted, as the security service reports it.";
    const COLUMNS: &'static [&'static str] = &["Role", "Account", "Entitlements"];
    const EMPTY: &'static str = "No authorities are defined.";
    fn read() -> RowsRead {
        RowsRead::portal("settings-authorities")
    }
}

/// `/video` — the Mux video test (SUPPORT, public URL kept).
pub struct VideoTest;

impl RowsSpec for VideoTest {
    const EYEBROW: &'static str = "Support";
    const TITLE: &'static str = "Mux Video Test";
    const PURPOSE: &'static str = "The published videos the Mux integration serves.";
    const COLUMNS: &'static [&'static str] = &["Video", "Detail"];
    const EMPTY: &'static str = "No videos are published through Mux.";
    fn read() -> RowsRead {
        RowsRead::public("site-video")
    }
}

/// `/review/:token/:page` — the token review page (SUPPORT, public URL kept: it may have been sent in email).
pub struct Review;

impl RowsSpec for Review {
    const EYEBROW: &'static str = "Review";
    const TITLE: &'static str = "Review";
    const PURPOSE: &'static str = "The items shared with you for review.";
    const COLUMNS: &'static [&'static str] = &["Item", "Detail"];
    const EMPTY: &'static str = "There is nothing to review at this link.";
    fn read() -> RowsRead {
        RowsRead::public("review")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::rows::RowsScreen;
    use crate::app::screen::{Screen, ScreenCtx};

    #[test]
    fn each_table_reads_its_own_rows_and_shows_them() {
        let ctx = ScreenCtx::default();
        let paths = [
            RowsScreen::<Roles>::init(&ctx).1,
            RowsScreen::<Authorities>::init(&ctx).1,
            RowsScreen::<VideoTest>::init(&ctx).1,
            RowsScreen::<Review>::init(&ctx).1,
        ]
        .map(|cmd| cmd.into_requests().remove(0).path);
        assert_eq!(
            paths,
            [
                "/api/portal/rust-ui/rows?screen=settings-roles",
                "/api/portal/rust-ui/rows?screen=settings-authorities",
                "/api/rust-ui/public-rows?screen=site-video",
                "/api/rust-ui/public-rows?screen=review",
            ]
        );
        let (mut model, cmd) = RowsScreen::<Roles>::init(&ctx);
        let answer = serde_json::json!([{ "id": "item-0", "cells": ["agent", "internal", "33"] }]);
        RowsScreen::<Roles>::update(
            &mut model,
            cmd.into_requests().remove(0).respond(Ok(answer)),
            &ctx,
        );
        assert_eq!(
            model.rows.loaded().map(|rows| rows[0].cells.len()),
            Some(Roles::COLUMNS.len())
        );
    }
}
