//! The Yew application: the Model, the reducer, and the mount point.//!
//! THE LOOP IS THE ONE THIS CRATE ALREADY HAD. `update(&mut Model, Msg)` is the only thing that changes application
//! state; this module owns a `Model`, turns Yew callbacks into `Msg`s, runs the `Effect`s the reducer returns, and
//! renders the model. There is no `use_state`, no second copy of the state, and no component that mutates anything: a
//! component receives the model and emits messages, exactly as the string renderers did.
//!
//! WHAT REPLACED THE OLD RUNTIME, and why none of it is here: no `set_inner_html` (Yew diffs), no manual DOM listeners
//! (callbacks), no global program in a thread-local (the component owns its model), no focus restoration (the DOM is
//! never thrown away), and no effect lifecycle in TypeScript (the fetch is an async function here). The generation still
//! exists, because a response must never mutate a screen it was not fetched for — and navigation is what asks for a new
//! screen, so navigation is where the new generation is stamped.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::model::{Msg, Screen};
use crate::yew_router::{Route, Shell};

/// The application's own message: the reducer's, plus the one thing only the browser knows.
pub enum AppMsg {
    /// An intent from a control — dispatched into the reducer unchanged.
    Ui(Msg),
    /// The URL changed and this screen is what it serves. Dynamic record routes carry their scope too.
    RouteEntered {
        screen: Screen,
        scope: Option<String>,
    },
}

/// The application. It owns the model, which is the whole of the app's state.
pub struct App {
    model: crate::model::Model,
    /// Which mount this is. Stamped on every request, and every response has to present it back.
    generation: u32,
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            model: crate::model::Model::default(),
            generation: 0,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let effects = match msg {
            AppMsg::Ui(msg) => crate::update::update(&mut self.model, msg),
            AppMsg::RouteEntered { screen, scope } => {
                // A NEW GENERATION PER NAVIGATION, which is what makes a late response harmless: the answer to the
                // previous screen's question cannot claim this one, however the two requests interleave.
                self.generation += 1;
                crate::update::update(
                    &mut self.model,
                    Msg::MountScoped {
                        screen,
                        scope,
                        generation: self.generation as u64,
                    },
                )
            }
        };
        for effect in effects {
            crate::yew_effects::run(effect, &ctx.link().callback(AppMsg::Ui));
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        // THE MASTER SHELL OWNS THE ROUTER AND THE CHROME (app/shell.rs). This old site app is hosted inside it for the
        // site screens not yet on the `Screen` trait, and is deleted when the last one is ported.
        html! {
            <Routed model={self.model.clone()} on_msg={ctx.link().callback(AppMsg::Ui)}
                on_route={ctx.link().callback(|(screen, scope)| AppMsg::RouteEntered { screen, scope })} />
        }
    }
}

#[derive(Properties, PartialEq)]
struct RoutedProps {
    model: crate::model::Model,
    on_msg: Callback<Msg>,
    on_route: Callback<(Screen, Option<String>)>,
}

/// Opens the screen the current URL serves, then renders it.
///
/// The announcement is an EFFECT rather than a side effect of rendering: rendering has to stay a function of the model,
/// so "the URL says Buyers now" is a message the app dispatches, not something a view does on its way past.
#[function_component(Routed)]
fn routed(props: &RoutedProps) -> Html {
    let route = use_route::<Route>().unwrap_or(Route::NotFound);
    {
        let on_route = props.on_route.clone();
        use_effect_with(route, move |route| {
            if let Some(screen) = route.screen() {
                on_route.emit((screen, route.scope()));
            }
            || ()
        });
    }
    html! {
        <Shell model={props.model.clone()} on_msg={props.on_msg.clone()} />
    }
}

/// The interruption, in the design's own words.
///
/// THE RETRY IS A LINK BACK TO THIS SAME URL, and that is a deliberate trade rather than an oversight. Next's error
/// boundary hands a client component a `reset()` that re-renders the segment in place; reaching it from here would mean
/// a callback crossing from React into this view, which is the kind of seam this port removes. An empty `href` resolves
/// to the current document, so the link reloads the page and retries — heavier than `reset`, always correct, and it
/// needs no JavaScript at all.
#[function_component(ErrorView)]
pub fn error_view() -> Html {
    html! {
        <section class="flex min-h-[80svh] items-center bg-foreground px-6 text-background md:px-12">
            <div class="mx-auto w-full max-w-[1600px]">
                <p class="mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/60">{"CulebraLuxe"}</p>
                <h1 class="max-w-2xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
                    {"Something drifted off course."}
                </h1>
                <p class="mt-6 max-w-xl text-pretty text-sm font-light leading-relaxed text-background/75">
                    {"A momentary interruption while we prepared this page. You can try again, or return to the CulebraLuxe home."}
                </p>
                <div class="mt-12 flex flex-wrap items-center gap-x-10 gap-y-4">
                    <a
                        href=""
                        class="group inline-flex min-h-11 items-center gap-3 border border-background/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] text-background transition-colors duration-500 hover:border-background"
                    >
                        {"Try again"}
                        <span class="inline-block h-px w-10 bg-background transition-all duration-500 group-hover:w-16" />
                    </a>
                    <a
                        href="/"
                        class="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.22em] text-background/80 transition-colors hover:text-background"
                    >
                        {"Return home"}
                        <span class="inline-block h-px w-8 bg-background/60 transition-all duration-500 group-hover:w-14" />
                    </a>
                </div>
            </div>
        </section>
    }
}
