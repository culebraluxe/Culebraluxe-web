//! The reducer: the only place the model changes, and it is pure.
//!
//! Navigation is a message like everything else, which is what keeps the shell dumb: a nav click, a deep link and a
//! restored session all arrive as `Navigate` and produce the same state.

use crate::model::{record_for, Controls, Effect, Model, Msg, Screen, PAGE_SIZE};

/// Whether a screen renders from a page payload rather than a list of rows.
///
/// This is the distinction the whole conversion turns on. A list screen answers "what rows are there" and renders them.
/// An editorial page answers "what are my blocks and cards" and lays them out — its hero has an image and an alt text,
/// its sections have eyebrows and calls to action. Asking for rows on a page like that is asking the wrong question,
/// and the answer is a page that renders as a list of strings with no design.
///
/// It is a small explicit list rather than a property of the surface, because "public" does not imply "editorial":
/// `/properties` is public and is a list.
pub fn is_editorial(key: &str) -> bool {
    // WHICH SCREENS ASK FOR A PAGE RATHER THAN ROWS. This list has to name every screen whose body renders blocks, and
    // on 2026-09-21 it did not name `site-services`: the Services page was ported, its route was wired, its payload was
    // served, and it still rendered an empty body, because opening it asked the host for ROWS. `model.page` stayed
    // `None`, the body function returned nothing, and the page was chrome over white. Nothing failed; nothing was empty
    // in a way anybody could see; the page was simply, silently, not the page.
    //
    // THE COUPLING IS THE HAZARD: a screen needs its page exactly when `custom_body` renders blocks for it, and the two
    // facts live in different files with nothing tying them together. If a screen renders blocks and is not named here,
    // it is blank; if it is named here and renders rows, it fetches a payload nobody reads. Both are invisible.
    matches!(
        key,
        "site-home"
            | "site-about"
            | "site-buyers"
            | "site-sellers"
            | "site-services"
            | "site-guide"
            | "site-contact"
            | "site-faq"
    )
}

/// Move to a screen and ask for its rows. The single place a screen change happens, so navigation and record-opening
/// cannot drift apart.
fn open(model: &mut Model, screen: Screen, scope: Option<String>) -> Vec<Effect> {
    model.screen = screen;
    model.scope = scope;
    // A deferred screen loads nothing: it is a placeholder, and pretending to fetch would put a spinner on a screen
    // that has no data to show.
    model.loading = !screen.is_deferred();
    model.error = None;
    // Rows belong to the screen that fetched them. Clearing on navigate is what stops a detail screen from briefly
    // rendering the previous screen's records.
    model.rows = Vec::new();
    model.selected_row_id = None;
    // The previous screen's blocks go with its rows: a page that has not loaded must not show the last one's hero.
    model.page = None;
    // Controls are the screen's own input and live their own life: a filter typed on Clients must not follow the user
    // to Deals and silently narrow a list they never filtered.
    model.controls = Controls::default();
    if model.loading {
        // A page asks for its blocks; a list asks for its rows. Two questions, two payloads, and the screen decides
        // which one it is asking — see `is_editorial`.
        if is_editorial(screen.key) {
            vec![Effect::FetchPage {
                screen: screen.key,
            }]
        } else {
            vec![Effect::FetchRows {
                screen: screen.key,
                scope: model.scope.clone(),
            }]
        }
    } else {
        Vec::new()
    }
}

/// Apply one intent. Returns the effects the host must run.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::ScreenOpened(screen) => open(model, screen, None),
        Msg::Navigate(screen) => {
            // Already there, and not deep inside a record: nothing to do. Coming *back* from a record with the same
            // screen needs the scope cleared, which is what the second half of the condition allows.
            if model.screen == screen && model.scope.is_none() {
                return Vec::new();
            }
            open(model, screen, None)
        }
        Msg::RecordOpened(id) => match record_for(model.screen.key) {
            Some(detail) => open(model, detail, Some(id)),
            // No detail screen: the same click means "select this one". An id that is not in the list is refused
            // rather than half-applied.
            None => {
                if model.rows.iter().any(|row| row.id == id) {
                    model.selected_row_id = Some(id);
                }
                Vec::new()
            }
        },
        Msg::RowsLoaded(rows) => {
            model.loading = false;
            model.error = None;
            if let Some(id) = model.selected_row_id.as_deref() {
                if !rows.iter().any(|row| row.id == id) {
                    model.selected_row_id = None;
                }
            }
            model.rows = rows;
            Vec::new()
        }
        Msg::RowSelected(id) => {
            // Selecting never writes and never fetches. An unknown id is refused rather than half-applied.
            if model.rows.iter().any(|row| row.id == id) {
                model.selected_row_id = Some(id);
            }
            Vec::new()
        }
        Msg::EffectFailed(message) => {
            model.loading = false;
            model.error = Some(message);
            Vec::new()
        }
        Msg::PageLoaded(page) => {
            model.loading = false;
            model.error = None;
            model.page = Some(page);
            Vec::new()
        }

        // ---- controls -------------------------------------------------------------------------------------------
        // None of these fetch. Filtering is applied to the rows already in the model, in the view, so it cannot be
        // mistaken for a server-side search that is not wired yet. When a filter does become a server round trip it
        // gains an effect here, and the host learns about it from the effect rather than from the message.
        Msg::QueryChanged(query) => {
            model.controls.query = query;
            // Page 4 of an unfiltered list means nothing once the list is not that list any more.
            model.controls.page = 0;
            Vec::new()
        }
        Msg::FilterChanged(filter) => {
            model.controls.filter = Some(filter);
            model.controls.page = 0;
            Vec::new()
        }
        Msg::TabSelected(tab) => {
            model.controls.tab = Some(tab);
            model.controls.page = 0;
            Vec::new()
        }
        Msg::Toggled(on) => {
            model.controls.toggled = on;
            Vec::new()
        }
        Msg::PageChanged(delta) => {
            // The bounds live here rather than in the buttons, so a list that shrank while the user was reading it
            // cannot leave them on a page that no longer exists.
            let next = (model.controls.page as i64).saturating_add(delta).max(0);
            let pages = model.rows.len().div_ceil(PAGE_SIZE);
            model.controls.page = if pages == 0 {
                // No rows in the model means the screen renders its body from somewhere the reducer cannot see (the
                // Rust design lab builds its own list). It refuses to go below the first page and the body clamps the
                // rest, rather than inventing a last page it has no count for.
                next as usize
            } else {
                next.min(pages as i64 - 1) as usize
            };
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Row, Screen};

    /// Screens are addressed by KEY in these tests: the table is the source of truth, so a test that named a variant
    /// would be asserting a name that only exists in a previous version of this file.
    fn target(key: &str) -> Screen {
        crate::model::screen(key).expect("a screen the table defines")
    }

    fn row(id: &str) -> Row {
        Row {
            id: id.into(),
            cells: vec![format!("row {id}")],
            badge: None,
        }
    }

    #[test]
    fn navigating_to_a_menu_screen_fetches_its_rows() {
        let mut model = Model::default();
        let effects = update(&mut model, Msg::Navigate(target("clients")));
        assert_eq!(model.screen, target("clients"));
        assert!(model.loading);
        assert_eq!(
            effects,
            vec![Effect::FetchRows {
                screen: "clients",
                scope: None
            }]
        );
    }

    #[test]
    fn navigating_back_to_the_same_screen_does_nothing() {
        let mut model = Model {
            screen: target("clients"),
            ..Model::default()
        };
        assert!(update(&mut model, Msg::Navigate(target("clients"))).is_empty());
    }

    #[test]
    fn the_deferred_screen_navigates_without_fetching() {
        let mut model = Model::default();
        // The receipt scanner is the screen that is a placeholder BY DESIGN (its own header calls it FAKE V1), so it is
        // the honest example now that Projects is wired.
        assert!(update(
            &mut model,
            Msg::Navigate(target("accounting-receipt-scanner"))
        )
        .is_empty());
        assert_eq!(model.screen, target("accounting-receipt-scanner"));
        assert!(
            !model.loading,
            "a placeholder must not show a spinner for data it never asks for"
        );
    }

    #[test]
    fn opening_a_listing_row_opens_that_record_and_asks_about_it() {
        let mut model = Model {
            screen: target("site-properties"),
            ..Model::default()
        };
        let effects = update(&mut model, Msg::RecordOpened("villa-del-mar".into()));
        assert_eq!(model.screen, target("site-property-detail"));
        assert_eq!(
            effects,
            vec![Effect::FetchRows {
                screen: "site-property-detail",
                scope: Some("villa-del-mar".into()),
            }],
            "the record key must reach the host, and a detail screen fetches about one record"
        );
    }

    #[test]
    fn opening_a_row_where_there_is_no_detail_view_selects_it() {
        // Activity is a feed: a row of it is history, not a record to open.
        let mut model = Model {
            screen: target("activity"),
            ..Model::default()
        };
        update(&mut model, Msg::RowsLoaded(vec![row("a")]));
        assert!(update(&mut model, Msg::RecordOpened("a".into())).is_empty());
        assert_eq!(
            model.screen,
            target("activity"),
            "there is nowhere to navigate to"
        );
        assert_eq!(model.selected_row_id.as_deref(), Some("a"));
    }

    #[test]
    fn navigating_back_to_a_list_clears_the_record_it_was_about() {
        let mut model = Model::default();
        update(&mut model, Msg::RecordOpened("villa-del-mar".into()));
        update(&mut model, Msg::Navigate(target("site-properties")));
        assert_eq!(
            model.scope, None,
            "a stale slug would make the next detail fetch about the wrong record"
        );
    }

    #[test]
    fn rows_from_the_previous_screen_never_leak_into_the_next() {
        let mut model = Model::default();
        update(&mut model, Msg::RowsLoaded(vec![row("a")]));
        update(&mut model, Msg::RowSelected("a".into()));
        update(&mut model, Msg::Navigate(target("deals")));
        assert!(model.rows.is_empty());
        assert_eq!(model.selected_row_id, None);
    }

    #[test]
    fn a_refresh_that_loses_the_selected_row_drops_the_selection() {
        let mut model = Model::default();
        update(&mut model, Msg::RowsLoaded(vec![row("a")]));
        update(&mut model, Msg::RowSelected("a".into()));
        update(&mut model, Msg::RowsLoaded(vec![row("b")]));
        assert_eq!(model.selected_row_id, None);
    }

    #[test]
    fn a_broken_payload_is_an_error_not_a_panic() {
        assert!(matches!(
            Msg::rows_loaded_json("nope"),
            Msg::EffectFailed(_)
        ));
    }

    #[test]
    fn a_failed_request_keeps_the_data_the_user_was_reading() {
        let mut model = Model {
            loading: true,
            ..Model::default()
        };
        update(&mut model, Msg::RowsLoaded(vec![row("a")]));
        model.loading = true;
        update(&mut model, Msg::EffectFailed("network".into()));
        assert_eq!(model.error.as_deref(), Some("network"));
        assert_eq!(model.rows.len(), 1);
    }

    // ---- controls ----------------------------------------------------------------------------------------------

    #[test]
    fn a_narrowing_control_returns_to_the_first_page() {
        let mut model = Model::default();
        update(
            &mut model,
            Msg::RowsLoaded((0..PAGE_SIZE + 5).map(|i| row(&i.to_string())).collect()),
        );
        update(&mut model, Msg::PageChanged(1));
        assert_eq!(model.controls.page, 1);
        update(&mut model, Msg::QueryChanged("ada".into()));
        assert_eq!(
            model.controls.page, 0,
            "page 2 of a list is not page 2 of the list the user is now filtering"
        );
    }

    #[test]
    fn paging_is_bounded_by_the_rows_the_model_holds() {
        let mut model = Model::default();
        update(
            &mut model,
            Msg::RowsLoaded((0..PAGE_SIZE + 1).map(|i| row(&i.to_string())).collect()),
        );
        update(&mut model, Msg::PageChanged(-1));
        assert_eq!(model.controls.page, 0, "there is no page before the first");
        update(&mut model, Msg::PageChanged(9));
        assert_eq!(
            model.controls.page, 1,
            "PAGE_SIZE+1 rows are two pages, so the last page is 1"
        );
    }

    #[test]
    fn filtering_does_not_follow_the_user_to_the_next_screen() {
        let mut model = Model::default();
        update(&mut model, Msg::QueryChanged("ada".into()));
        update(&mut model, Msg::TabSelected("open".into()));
        update(&mut model, Msg::Toggled(true));
        update(&mut model, Msg::Navigate(target("deals")));
        assert_eq!(
            model.controls,
            Controls::default(),
            "a filter typed on one screen must not narrow the next one"
        );
    }

    #[test]
    fn a_control_message_asks_the_host_for_nothing() {
        let mut model = Model::default();
        for msg in [
            Msg::QueryChanged("x".into()),
            Msg::FilterChanged("open".into()),
            Msg::TabSelected("all".into()),
            Msg::Toggled(true),
            Msg::PageChanged(1),
        ] {
            assert!(
                update(&mut model, msg).is_empty(),
                "filtering is local until a screen's filter becomes a server round trip, and then it earns an effect"
            );
        }
    }
}
