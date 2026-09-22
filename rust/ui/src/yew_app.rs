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
    /// The URL changed and this screen is what it serves: opened under a new generation.
    RouteEntered(Screen),
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
            AppMsg::RouteEntered(screen) => {
                // A NEW GENERATION PER NAVIGATION, which is what makes a late response harmless: the answer to the
                // previous screen's question cannot claim this one, however the two requests interleave.
                self.generation += 1;
                crate::update::update(
                    &mut self.model,
                    Msg::Mount {
                        screen,
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
        html! {
            <BrowserRouter>
                <Routed model={self.model.clone()} on_msg={ctx.link().callback(AppMsg::Ui)}
                    on_route={ctx.link().callback(AppMsg::RouteEntered)} />
            </BrowserRouter>
        }
    }
}

#[derive(Properties, PartialEq)]
struct RoutedProps {
    model: crate::model::Model,
    on_msg: Callback<Msg>,
    on_route: Callback<Screen>,
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
                on_route.emit(screen);
            }
            || ()
        });
    }
    html! {
        <Shell model={props.model.clone()} on_msg={props.on_msg.clone()} />
    }
}

/// Mount the Yew application into `element_id`.
///
/// THE ENTRY POINT A PAGE CALLS, and the only thing the browser needs from this crate now: the module boots, this
/// function takes the container, and from there the router owns the URL — so a deep link, a refresh and the back button
/// are the router's business rather than a `start` prop a page has to work out.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn yew_mount(element_id: &str) -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("ui: no document"))?;
    let root = document.get_element_by_id(element_id).ok_or_else(|| {
        wasm_bindgen::JsValue::from_str(&format!("ui: no element '{element_id}'"))
    })?;
    yew::Renderer::<App>::with_root(root).render();
    Ok(())
}

