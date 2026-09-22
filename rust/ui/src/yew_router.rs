//! Routing, and the switch that turns a URL into a screen.
//!
//! YEW ROUTER OWNS THE PATHS THIS APP SERVES, and no others. Today that is `/buyers`: the other public routes are still
//! rendered by the string renderers through the Next host, and a path this bundle does not own must not resolve here —
//! otherwise one URL would be claimed by two runtimes at once. Porting another screen is one variant plus one view.
//!
//! THE ROUTE CARRIES A SCREEN KEY, not a screen of its own. `Route::screen()` resolves through the same registry the MVI
//! model uses, so a URL cannot name a page the model does not know, and the payload a screen fetches is the payload its
//! registry entry says.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::model::{screen, Msg, Screen};
use crate::yew_views::buyers::Buyers;
use crate::yew_views::services::Services;

/// The public paths this app owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Routable)]
pub enum Route {
    #[at("/buyers")]
    Buyers,
    #[at("/services")]
    Services,
    /// A URL this app does not serve. It renders a message rather than a blank page, and the reader is offered the way
    /// back to the home page — the honest answer to a path that has no screen here.
    #[not_found]
    #[at("/404")]
    NotFound,
}

impl Route {
    /// The screen this URL serves, from the registry the model and the payload routes already share.
    pub fn screen(self) -> Option<Screen> {
        match self {
            Route::Buyers => screen("site-buyers"),
            Route::Services => screen("site-services"),
            Route::NotFound => None,
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct ShellProps {
    /// The screen's state. Cloned into the route component that renders, because a Yew component owns what it is given.
    pub model: crate::model::Model,
    /// Every control's intent, dispatched into the one reducer.
    pub on_msg: Callback<Msg>,
}

/// The application: the public chrome, and the screen the current URL serves.
pub struct Shell;

impl Component for Shell {
    type Message = ();
    type Properties = ShellProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let model = props.model.clone();
        let on_msg = props.on_msg.clone();
        html! {
            <div class="flex min-h-screen flex-col bg-background text-foreground">
                <crate::yew_views::chrome::Header />
                <main class="min-w-0 flex-1">
                    <Switch<Route> render={Callback::from(move |route: Route| match route {
                        Route::Buyers => html! { <Buyers model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Services => html! { <Services model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::NotFound => html! { <NotFound /> },
                    })} />
                </main>
                <crate::yew_views::chrome::Footer />
            </div>
        }
    }
}

/// A path this app does not own.
#[function_component(NotFound)]
fn not_found() -> Html {
    html! {
        <section class="px-6 py-32 md:px-12">
            <div class="mx-auto max-w-3xl">
                <p class="mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent">{"Not found"}</p>
                <h1 class="font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                    {"That page is not served here."}
                </h1>
                <p class="mt-6 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
                    {"The address you followed does not belong to this part of the site."}
                </p>
                <a href="/" class="mt-8 inline-flex text-xs font-light uppercase tracking-[0.2em] text-accent">
                    {"Return to the home page"}
                </a>
            </div>
        </section>
    }
}
