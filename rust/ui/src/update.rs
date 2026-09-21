//! The reducer: the only place the model changes, and it is pure.
//!
//! Navigation is a message like everything else, which is what keeps the shell dumb: a nav click, a deep link and a
//! restored session all arrive as `Navigate` and produce the same state.

use crate::model::{record_for, Effect, Model, Msg, Screen};

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
    if model.loading {
        vec![Effect::FetchRows {
            screen: screen.key,
            scope: model.scope.clone(),
        }]
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
        assert!(update(&mut model, Msg::Navigate(target("projects"))).is_empty());
        assert_eq!(model.screen, target("projects"));
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
}
