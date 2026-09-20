//! The reducer: the only place the model changes, and it is pure.
//!
//! Navigation is a message like everything else, which is what keeps the shell dumb: a nav click, a deep link and a
//! restored session all arrive as `Navigate` and produce the same state.

use crate::model::{Effect, Model, Msg};

/// Apply one intent. Returns the effects the host must run.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::ScreenOpened(screen) => {
            model.screen = screen;
            model.loading = true;
            model.error = None;
            // Rows belong to the screen that fetched them. Clearing on navigate is what stops a detail panel from
            // briefly rendering the previous screen's records.
            model.rows = Vec::new();
            model.selected_row_id = None;
            vec![Effect::FetchRows]
        }
        Msg::Navigate(screen) => {
            if model.screen == screen {
                return Vec::new();
            }
            // A deferred screen loads nothing: it is a placeholder, and pretending to fetch would put a spinner on a
            // screen that has no data to show.
            if screen.is_deferred() {
                model.screen = screen;
                model.loading = false;
                model.error = None;
                model.rows = Vec::new();
                model.selected_row_id = None;
                return Vec::new();
            }
            update(model, Msg::ScreenOpened(screen))
        }
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
        let effects = update(&mut model, Msg::Navigate(Screen::Clients));
        assert_eq!(model.screen, Screen::Clients);
        assert!(model.loading);
        assert_eq!(effects, vec![Effect::FetchRows]);
    }

    #[test]
    fn navigating_back_to_the_same_screen_does_nothing() {
        let mut model = Model {
            screen: Screen::Clients,
            ..Model::default()
        };
        assert!(update(&mut model, Msg::Navigate(Screen::Clients)).is_empty());
    }

    #[test]
    fn the_deferred_screen_navigates_without_fetching() {
        let mut model = Model::default();
        assert!(update(&mut model, Msg::Navigate(Screen::Projects)).is_empty());
        assert_eq!(model.screen, Screen::Projects);
        assert!(
            !model.loading,
            "a placeholder must not show a spinner for data it never asks for"
        );
    }

    #[test]
    fn rows_from_the_previous_screen_never_leak_into_the_next() {
        let mut model = Model::default();
        update(&mut model, Msg::RowsLoaded(vec![row("a")]));
        update(&mut model, Msg::RowSelected("a".into()));
        update(&mut model, Msg::Navigate(Screen::Deals));
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
