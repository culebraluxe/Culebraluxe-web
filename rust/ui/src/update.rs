//! The reducer: the only place the model changes, and it is pure.
//!
//! Everything that could go wrong with an MVI screen goes wrong here instead of in a lifecycle hook: a stale
//! response, a selection pointing at a row that no longer exists, an error arriving while a new request is in
//! flight. Each of those is a named message and an assertion below.

use crate::model::{Effect, Model, Msg};

/// Apply one intent. Returns the effects the host must run.
///
/// The model is updated before the effects are returned, so the view is always rendering a valid state and a
/// request is never a precondition for showing something.
pub fn update(model: &mut Model, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::ScreenOpened => {
            model.loading = true;
            model.error = None;
            vec![Effect::LoadPortfolio]
        }
        Msg::PortfolioLoaded(properties) => {
            model.loading = false;
            model.error = None;
            // A selection that survives the refresh stays; one that does not is dropped rather than left dangling,
            // because a dangling id would make `selected()` return None while the UI still showed a detail screen.
            if let Some(id) = model.selected_property_id.as_deref() {
                if !properties.iter().any(|property| property.id == id) {
                    model.selected_property_id = None;
                    model.screen = crate::model::Screen::Portfolio;
                }
            }
            model.properties = properties;
            Vec::new()
        }
        Msg::PropertySelected(id) => {
            // Selecting is a view concern: it never loads, never writes, never asks the server. If the id is not in
            // the model the selection is refused, which is what stops a widget from selecting a row the screen does
            // not know about.
            if model.properties.iter().any(|property| property.id == id) {
                model.selected_property_id = Some(id);
                model.screen = crate::model::Screen::PropertyDetail;
            }
            Vec::new()
        }
        Msg::PortfolioRequested => {
            model.screen = crate::model::Screen::Portfolio;
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
    use crate::model::{PropertySummary, Screen};

    fn summary(id: &str, label: &str) -> PropertySummary {
        PropertySummary {
            id: id.into(),
            label: label.into(),
            owner_known: true,
            acreage: Some(1.0),
        }
    }

    #[test]
    fn opening_the_screen_loads_and_clears_a_previous_error() {
        let mut model = Model {
            error: Some("old".into()),
            ..Model::default()
        };
        let effects = update(&mut model, Msg::ScreenOpened);
        assert_eq!(effects, vec![Effect::LoadPortfolio]);
        assert!(model.loading);
        assert_eq!(model.error, None);
    }

    #[test]
    fn loading_data_stops_the_spinner() {
        let mut model = Model {
            loading: true,
            ..Model::default()
        };
        update(&mut model, Msg::PortfolioLoaded(vec![summary("p1", "One")]));
        assert!(!model.loading);
        assert_eq!(model.properties.len(), 1);
    }

    #[test]
    fn selecting_a_known_property_opens_the_detail_screen() {
        let mut model = Model {
            properties: vec![summary("p1", "One")],
            ..Model::default()
        };
        update(&mut model, Msg::PropertySelected("p1".into()));
        assert_eq!(model.selected_property_id.as_deref(), Some("p1"));
        assert_eq!(model.screen, Screen::PropertyDetail);
        assert_eq!(model.selected().map(|p| p.label.as_str()), Some("One"));
    }

    #[test]
    fn selecting_an_unknown_property_is_refused_rather_than_half_applied() {
        let mut model = Model {
            properties: vec![summary("p1", "One")],
            ..Model::default()
        };
        update(&mut model, Msg::PropertySelected("nope".into()));
        assert_eq!(model.selected_property_id, None);
        assert_eq!(model.screen, Screen::Portfolio);
    }

    #[test]
    fn a_refresh_that_loses_the_selected_row_closes_the_detail_screen() {
        let mut model = Model {
            properties: vec![summary("p1", "One")],
            selected_property_id: Some("p1".into()),
            screen: Screen::PropertyDetail,
            ..Model::default()
        };
        update(&mut model, Msg::PortfolioLoaded(vec![summary("p2", "Two")]));
        assert_eq!(model.selected_property_id, None);
        assert_eq!(
            model.screen,
            Screen::Portfolio,
            "a dangling selection must not leave a detail screen open"
        );
    }

    #[test]
    fn a_failed_request_keeps_the_data_and_records_the_reason() {
        let mut model = Model {
            loading: true,
            properties: vec![summary("p1", "One")],
            ..Model::default()
        };
        update(&mut model, Msg::EffectFailed("network".into()));
        assert!(!model.loading);
        assert_eq!(model.error.as_deref(), Some("network"));
        assert_eq!(
            model.properties.len(),
            1,
            "an error never discards data the user could still be reading"
        );
    }

    #[test]
    fn going_back_keeps_the_selection_so_returning_is_cheap() {
        let mut model = Model {
            properties: vec![summary("p1", "One")],
            selected_property_id: Some("p1".into()),
            screen: Screen::PropertyDetail,
            ..Model::default()
        };
        update(&mut model, Msg::PortfolioRequested);
        assert_eq!(model.screen, Screen::Portfolio);
        assert_eq!(model.selected_property_id.as_deref(), Some("p1"));
    }
}
