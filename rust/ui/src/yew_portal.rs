//! The Portal application, on Yew: the model, the reducer, and the browser entry point for a portal page.
//!
//! THE SAME LOOP AS THE PUBLIC APP, and deliberately a separate root: a portal page mounts ITS screen (the URL belongs
//! to Next), where the public app mounts a router that owns five paths. Both own a `Model` and dispatch into the one
//! `update`, so there is still exactly one reducer and one state.

use yew::prelude::*;

use crate::model::Msg;
use crate::yew_views::portal_activity::Activity;

/// The application's own message: the reducer's, plus the one thing only the browser knows.
pub enum AppMsg {
    Ui(Msg),
}

pub struct PortalApp {
    model: crate::model::Model,
    /// Which mount this is. Stamped on every request, and every response presents it back before it is applied.
    generation: u32,
}

impl Component for PortalApp {
    type Message = AppMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            model: crate::model::Model::default(),
            generation: 0,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let AppMsg::Ui(msg) = msg;
        let effects = crate::update::update(&mut self.model, msg);
        for effect in effects {
            crate::yew_effects::run(effect, &ctx.link().callback(AppMsg::Ui));
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_msg = ctx.link().callback(AppMsg::Ui);
        html! {
            <Activity model={self.model.clone()} on_msg={on_msg} />
        }
    }
}

/// Mount the portal application for `screen` into `element_id`.
///
/// The screen key comes from the page because the page owns the URL; the generator is stamped on the mount so a response
/// can be matched to the run that asked for it.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn portal_mount(element_id: &str, screen_key: &str) -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let screen = crate::model::screen(screen_key)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str(&format!("ui: '{screen_key}' is not a known screen")))?;
    if !crate::update::is_ported_portal_screen(screen.key) {
        // A screen with no Yew component must not half-mount: the honest answer is to say so rather than paint a shell
        // around an empty body.
        return Err(wasm_bindgen::JsValue::from_str(&format!(
            "ui: '{screen_key}' has no Yew component yet"
        )));
    }
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("ui: no document"))?;
    let root = document
        .get_element_by_id(element_id)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str(&format!("ui: no element '{element_id}'")))?;
    yew::Renderer::<PortalApp>::with_root(root).render();
    Ok(())
}
