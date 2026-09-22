//! The Portal, on Yew: its own shell, built from the `SCREENS` registry.
//!
//! THE PUBLIC CHROME IS NOT FORCED ONTO THE PORTAL. The public site is a website with a header and a footer; the portal
//! is an application with a fixed rail of sections, and putting one's chrome on the other is worse than a plain page —
//! the shape of a screen is the first thing a reader takes from it. So this shell is its own, and it reads the same
//! registry the model and the navigation already read.
//!
//! ONE SCREEN PER MOUNTED PAGE, as the public side works: the Next route owns its URL, hands the screen key in, and the
//! shell marks it in the rail. A rail entry for a screen with no Yew component yet is an ordinary anchor, so its route
//! keeps working while the port goes screen by screen; the ones that are ported navigate by intent.

use yew::prelude::*;

use crate::model::{home, screen, Msg, Screen, Surface, SCREENS};

/// What a portal page hands the shell: which screen it is, the model, and how to dispatch.
#[derive(Properties, PartialEq)]
pub struct PortalShellProps {
    /// The screen the page serves, from the registry.
    pub screen: Screen,
    /// The model, which owns everything the screen renders.
    pub model: crate::model::Model,
    /// Every intent, dispatched into the one reducer.
    pub on_msg: Callback<Msg>,
    /// The screen's body.
    pub children: Children,
}

/// The portal's chrome: the section rail for the current surface, the surfaces, and the screen.
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
        let surface = current.surface;
        let on_msg = props.on_msg.clone();
        // The rail is the registry's, filtered to the surface the screen belongs to — the same rule the string shell
        // followed, so "what navigation belongs under a surface" is answered in one place.
        let sections = SCREENS
            .iter()
            .copied()
            .filter(|candidate| candidate.surface == surface && candidate.is_listed())
            .collect::<Vec<_>>();
        let loading = props.model.loading;
        let error = props.model.error.clone();
        html! {
            <div class="flex min-h-screen text-foreground" data-rust-screen={current.key}>
                <nav class="w-60 shrink-0 border-r bg-card p-4" aria-label="Portal">
                    <p class="mb-2 px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        { surface.label() }
                    </p>
                    { for sections.iter().map(|candidate| self.rail_entry(*candidate, current, &on_msg)) }
                    <p class="mt-6 mb-2 px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">{"Surfaces"}</p>
                    { for Surface::ALL.iter().copied().filter_map(|candidate| home(candidate).map(|target| (candidate, target))).map(|(candidate, target)| {
                        let active = candidate == surface;
                        html! {
                            <a href={target.path.to_string()} title={format!("home: {}", target.key)}
                                class={classes!("block", "w-full", "rounded-md", "px-2", "py-1", "text-left", "text-xs",
                                    if active { "text-foreground font-medium" } else { "text-muted-foreground hover:bg-muted/60" })}>
                                { candidate.label() }
                            </a>
                        }
                    }) }
                </nav>
                <main class="min-w-0 flex-1 p-6">
                    if let Some(message) = error {
                        <div class="mb-4 rounded-md border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm" role="alert">
                            { message }
                        </div>
                    }
                    if loading {
                        <p class="mb-4 text-sm text-muted-foreground" data-portal-status="loading">{"Loading…"}</p>
                    }
                    { for props.children.iter() }
                </main>
            </div>
        }
    }
}

impl PortalShell {
    /// Portal URLs still belong to Next while the Yew conversion is screen-by-screen.
    ///
    /// Keep every rail entry as a real anchor until the Portal itself owns routing. A button that only dispatched
    /// Msg::Navigate changed the Rust model without changing the browser URL, which breaks refresh, back/forward and
    /// deep-link truth. Full document navigation is the honest bridge during this staged cutover.
    fn rail_entry(&self, candidate: Screen, current: Screen, _on_msg: &Callback<Msg>) -> Html {
        let active = candidate == current;
        let class = classes!(
            "block", "w-full", "rounded-md", "px-2", "py-1.5", "text-left", "text-sm",
            if active { "bg-muted font-medium" } else { "hover:bg-muted/60 text-muted-foreground" }
        );
        html! {
            <a href={candidate.path.to_string()} aria-current={active.then_some("page")} {class}>
                { candidate.title }
                if candidate.is_deferred() { {" (no data yet)"} }
            </a>
        }
    }
}
