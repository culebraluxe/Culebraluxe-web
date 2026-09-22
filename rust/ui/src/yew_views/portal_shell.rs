//! Portal body wrapper for Yew screens.
//!
//! The real Portal chrome is owned by app/portal/layout.tsx -> OperatingShell.
//! This component must never invent a second navigation rail.

use yew::prelude::*;

use crate::model::{Msg, Screen};

#[derive(Properties, PartialEq)]
pub struct PortalShellProps {
    pub screen: Screen,
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
    #[prop_or_default]
    pub children: Children,
}

pub struct PortalShell;

impl Component for PortalShell {
    type Message = ();
    type Properties = PortalShellProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let current = props.screen;
        let loading = props.model.loading;
        let error = props.model.error.clone();

        html! {
            <div class="min-w-0" data-rust-screen={current.key}>
                if let Some(message) = error {
                    <div class="mb-4 rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm" role="alert">
                        { message }
                    </div>
                }
                if loading {
                    <p class="mb-4 text-sm text-muted-foreground" data-portal-status="loading">{"Loading…"}</p>
                }
                { for props.children.iter() }
            </div>
        }
    }
}
