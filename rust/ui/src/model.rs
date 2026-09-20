//! The Model: everything the screen is, in one place.

/// A property as the list needs it. Deliberately small and owned by the model rather than borrowed from a response
/// type, so the view cannot accidentally depend on a wire shape.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertySummary {
    pub id: String,
    pub label: String,
    /// Whether the warehouse knows who owns it. The Regrid work made this a real distinction: 378 parcels have no
    /// owner of record, and the UI should say so rather than show a blank.
    pub owner_known: bool,
    pub acreage: Option<f64>,
}

/// Which screen the shell should be showing. One variant per screen, and the Project Management screen is last on
/// purpose — it is the one that has to wrap third-party widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Portfolio,
    PropertyDetail,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Model {
    pub screen: Screen,
    pub loading: bool,
    pub properties: Vec<PropertySummary>,
    pub selected_property_id: Option<String>,
    /// Set when the last request failed. A rejected change lands here so the view can show it and any widget bound
    /// to the same selection can snap back to the model's value.
    pub error: Option<String>,
}

impl Default for Screen {
    fn default() -> Self {
        Self::Portfolio
    }
}

impl Model {
    /// The property the detail screen is showing, if the selection still points at one. Selection is an id, never an
    /// index or a copied struct: a refreshed list must not silently re-point the selection at a different row.
    pub fn selected(&self) -> Option<&PropertySummary> {
        let id = self.selected_property_id.as_deref()?;
        self.properties.iter().find(|property| property.id == id)
    }
}

/// Every intent the screen can receive. This is the whole vocabulary — a widget can only ever tell the model one of
/// these, which is what keeps a third-party library from growing its own idea of application state.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// The screen mounted: load what it needs.
    ScreenOpened,
    PortfolioLoaded(Vec<PropertySummary>),
    /// A row was chosen, by a click in the Rust view or by a selection event from a widget.
    PropertySelected(String),
    /// The user asked to go back to the list.
    PortfolioRequested,
    /// The host reports that a request failed. The model keeps its previous data; the message is the record.
    EffectFailed(String),
}

/// What the host has to do after a state change. Effects are requests, never decisions: the model asks to load, it
/// does not decide whether it may.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    LoadPortfolio,
}
