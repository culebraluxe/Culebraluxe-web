//! The browser shell — the only place in this crate that knows a DOM exists.
//!
//! IT DOES TWO THINGS. It hands the application the container element, and it turns what happens in that container into
//! a `Msg`: the listeners below read `data-` attributes off the nearest ancestor of a click or a keystroke and deliver
//! the intent to the ONE application that owns the container. It does not render, and it does not repaint — the
//! application diffs its own DOM.
//!
//! WHY THE LISTENERS EXIST AT ALL. Markup the application renders as a string (`StringBody`, `SiteBody`) has no Yew
//! callbacks: its controls ARE attributes. Without these listeners the nav, the tables, the filters and the tabs in those
//! bodies would be inert.
//!
//! THREE DECISIONS WORTH KNOWING, all of them the kind that bite later if left implicit:
//!
//! * **The listeners are leaked on purpose.** `Closure::forget` keeps them alive for the life of the page. The
//!   alternative is a `static` holding them, which is the same lifetime with more ceremony. Dropping the closure is the
//!   classic way a Rust WASM UI works once and then goes deaf.
//! * **The container is a thread-local, written by `start_in` and read per event.** WASM is single-threaded here,
//!   `Rc<RefCell<_>>` is not `Sync`, and a listener must act on the container that is live NOW rather than one captured
//!   when it was installed.
//! * **Nothing here performs a request.** The application fetches through the browser's own routes, which hold the
//!   session and the server-side credentials. No internal key exists anywhere in this module.
#![cfg(feature = "wasm")]

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Element, Event, HtmlElement, HtmlInputElement, HtmlSelectElement, MouseEvent, Window,
};

use crate::{home, screen, screen_for_path, Msg, Surface};

const ATTRIBUTE_NAV: &str = "data-nav";
const ATTRIBUTE_SURFACE: &str = "data-surface";
const ATTRIBUTE_SELECT_ROW: &str = "data-select-row";
const ATTRIBUTE_OPEN_RECORD: &str = "data-open-record";
/// The input controls. Each name is the intent the shell turns it into, and the mapping is written out rather than
/// derived from the attribute: an unknown field is ignored instead of dispatching a message nobody meant.
const ATTRIBUTE_FIELD: &str = "data-field";
const ATTRIBUTE_SELECT: &str = "data-select";
const ATTRIBUTE_TOGGLE: &str = "data-toggle";
const ATTRIBUTE_TAB: &str = "data-tab";
const ATTRIBUTE_PAGE: &str = "data-page";
const ATTRIBUTE_CLEAR: &str = "data-clear";

thread_local! {
    /// The container the application was mounted in, as the page handed it over.
    ///
    /// STORED, NOT LOOKED UP. `#rust-ui` is an id, and during a client-side navigation Next has the new route and the
    /// old one in the document at once, so a lookup answers with the page being left. `start_in` is given the node the
    /// page owns and keeps it here.
    static ROOT: RefCell<Option<Element>> = const { RefCell::new(None) };
    /// Whether the document listeners are already installed.
    ///
    /// THE PAGE MOUNTS ON EVERY EFFECT RUN, and React runs effects twice in development. Installing the listeners each
    /// time would mean two of each on the document, so a single click would dispatch its intent twice — a double
    /// navigation, or a row opened twice. They resolve the container per event, so a repeat mount installs nothing and
    /// breaks nothing.
    static LISTENERS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The seat the application receives its intents on.
    ///
    /// THIS IS WHERE "ONE OWNER" IS ENFORCED. The listeners below resolve a click or a keystroke into a `Msg`, and the
    /// message is delivered HERE — to the one application that owns the container. It is claimed by whichever
    /// application mounts and replaced on every mount, so an intent can never reach a model that has been destroyed.
    static YEW_DISPATCH: RefCell<Option<Rc<dyn Fn(Msg)>>> = const { RefCell::new(None) };
}

fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("ui: no window"))
}

fn document() -> Result<Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| JsValue::from_str("ui: no document"))
}

/// What the DOM is about to lose when `paint` replaces the markup.
///
/// WHY THIS EXISTS: focus and the caret are the browser's, not the model's — the model knows the text, not where the
/// cursor is. `set_inner_html` builds new elements every time, so without capturing and restoring these, a keystroke
/// would drop the caret to the end of the field or lose focus entirely, and the text field would be unusable however
/// correct the rest of the machinery was.
struct Focus {
    field: String,
    start: Option<u32>,
    end: Option<u32>,
}

fn capture_focus() -> Option<Focus> {
    let active = document().ok()?.active_element()?;
    let field = active.get_attribute(ATTRIBUTE_FIELD)?;
    let input = active.dyn_into::<HtmlInputElement>().ok()?;
    Some(Focus {
        field,
        start: input.selection_start().ok().flatten(),
        end: input.selection_end().ok().flatten(),
    })
}

fn restore_focus(focus: &Focus) {
    let Ok(document) = document() else { return };
    // The field name is one this crate rendered (`query`), so it needs no escaping beyond its own alphabet.
    let selector = format!("[{ATTRIBUTE_FIELD}=\"{}\"]", focus.field);
    let Ok(Some(element)) = document.query_selector(&selector) else {
        return;
    };
    let Ok(input) = element.dyn_into::<HtmlInputElement>() else {
        return;
    };
    let _ = input.focus();
    if let (Some(start), Some(end)) = (focus.start, focus.end) {
        let _ = input.set_selection_range(start, end);
    }
}

fn msg_for_input(field: &str, value: String) -> Option<Msg> {
    match field {
        "query" => Some(Msg::QueryChanged(value)),
        _ => None,
    }
}

/// The message a changed control means. `change` is what the browser fires for a dropdown or a checkbox once the user
/// has committed, which is why the model can be written from it directly rather than polling.
fn msg_for_change(element: &Element) -> Option<Msg> {
    if element.get_attribute(ATTRIBUTE_TOGGLE).is_some() {
        let input = element.clone().dyn_into::<HtmlInputElement>().ok()?;
        return Some(Msg::Toggled(input.checked()));
    }
    let name = element.get_attribute(ATTRIBUTE_SELECT)?;
    let select = element.clone().dyn_into::<HtmlSelectElement>().ok()?;
    match name.as_str() {
        "filter" => Some(Msg::FilterChanged(select.value())),
        // THE NAMED DROPDOWNS. Listed one by one rather than passed through, for the same reason the fields are: an
        // unknown `data-select` is ignored instead of dispatching a name the model has no use for and cannot validate.
        // These three are the Buyers inventory bar's.
        "price" | "beds" | "sort" => Some(Msg::FilterSelected {
            key: name,
            value: select.value(),
        }),
        _ => None,
    }
}

/// Install (or clear) the seat a Yew app receives intents on.
///
/// CALLED BY THE YEW APP ITSELF, from `PortalApp::create`, because the callback belongs to a component instance: the
/// component holds the model, so it must be the component that is told, and it replaces whatever a previous app left
/// here. The shell never reaches into a component and a component never paints; a `Msg` is the whole of the contract.
pub fn set_dispatcher(dispatcher: Option<Rc<dyn Fn(Msg)>>) {
    YEW_DISPATCH.with(|slot| *slot.borrow_mut() = dispatcher);
}

/// Deliver ONE intent to the ONE owner of the container.
///
/// THE ONLY PLACE AN INTENT CAN GO, which is what makes "one owner" a fact rather than an intention. The application
/// holds the model and diffs its own DOM, so nothing here repaints and nothing is captured — and there is no second seat
/// to fall back to, because there is no second renderer.
///
/// AN INTENT WITH NO OWNER IS DROPPED. A click that arrives after the application was destroyed has nowhere to go;
/// that is not an error worth raising.
///
/// FOCUS IS CAPTURED AND RESTORED AROUND IT. A body rendered as markup is re-rendered by replacing that markup, so the
/// field being typed in is rebuilt on every keystroke; restoring the caret here — in the one place every intent passes
/// through — is what keeps it usable.
fn deliver(msg: Msg) {
    let Some(dispatch) = YEW_DISPATCH.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let focus = capture_focus();
    dispatch(msg);
    if let Some(focus) = focus {
        restore_focus(&focus);
    }
}

/// The container the application was mounted in, as the page handed it over.
///
/// STORED BY `start_in`, READ PER EVENT. The listeners are installed once and must act on whatever container is live
/// now, so they ask for it at event time rather than capturing it: a listener that captured a container the page later
/// replaced would be attached to a detached node, and every control would be dead with nothing to say why.
fn current_root() -> Result<Element, JsValue> {
    ROOT.with(|slot| slot.borrow().clone())
        .ok_or_else(|| JsValue::from_str("ui: nothing is mounted yet"))
}

/// Whether an event's target is inside the current root.
///
/// The listeners are on the document, so they see clicks on the whole page: the header's own links, a third-party
/// widget, anything. An intent-bearing attribute found outside the application's container is not this crate's to act
/// on, and acting on it would let one thing on the page drive another.
fn inside_root(root: &Element, event: &Event) -> Option<Element> {
    let target = event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())?;
    root.contains(Some(&target)).then_some(target)
}

/// Install the document's click, input and change listeners, once per document.
///
/// THESE ARE WHAT MAKE MARKUP INTERACTIVE. A body the application renders as a string (`yew_portal::StringBody`,
/// `yew_router::SiteBody`) has no callbacks of its own — its buttons, tabs, filters and rows are `data-*` attributes —
/// so without these its controls would be dead, and the nav, the tables and the filters are exactly those attributes.
///
/// IDEMPOTENT BY THE FLAG, and it resolves the container and the owner per event (`current_root`, `deliver`), so calling
/// it from any number of mounts installs one set of listeners and breaks nothing.
pub fn listen() {
    if let Ok(document) = document() {
        let _ = install_document_listeners(&document);
    }
}

fn install_document_listeners(document: &Document) -> Result<(), JsValue> {
    if LISTENERS_INSTALLED.with(|flag| flag.replace(true)) {
        return Ok(());
    }
    let listener = {
        Closure::<dyn FnMut(MouseEvent)>::wrap(Box::new(move |event: MouseEvent| {
            // THE ROOT IS RESOLVED HERE, not captured when the listener was installed: a container React replaced must
            // still receive its clicks, and a listener on a detached node receives nothing. See `current_root`.
            let Ok(root) = current_root() else {
                return;
            };
            // And the click must be inside it: these listeners are on the document, so they see the whole page.
            let mut node = inside_root(&root, event.as_ref());
            let mut msg = None;
            while let Some(element) = node {
                if let Some(key) = element.get_attribute(ATTRIBUTE_SURFACE) {
                    // A surface is a destination, not a screen: the switcher offers the surface's home.
                    //
                    // THE HOME IS THE REGISTRY'S, NOT THE MODEL'S. This used `model::home`, which is the surface's
                    // FIRST LISTED SCREEN — and for Core that is `/portal/dashboard`, so every surface intent landed on
                    // the cockpit instead of the screen the menu actually points at. `crate::navigation` holds the
                    // registry's `home`; it is the same value the top-nav capsule links to, which is why the two can no
                    // longer disagree.
                    msg = Surface::from_key(&key)
                        .and_then(|surface| {
                            let path = crate::navigation::home_path(surface);
                            // The registry's route as a screen; if the model has no screen at that exact route, fall
                            // back to the surface's own home so the click still goes somewhere sensible rather than
                            // nowhere.
                            screen_for_path(path).or_else(|| home(surface))
                        })
                        .map(Msg::Navigate);
                    break;
                }
                if let Some(key) = element.get_attribute(ATTRIBUTE_NAV) {
                    msg = screen(&key).map(Msg::Navigate);
                    break;
                }
                if let Some(id) = element.get_attribute(ATTRIBUTE_OPEN_RECORD) {
                    // Checked before `data-select-row`, because a row that can be opened carries both attributes and
                    // opening is the stronger intent.
                    msg = Some(Msg::RecordOpened(id));
                    break;
                }
                if let Some(id) = element.get_attribute(ATTRIBUTE_SELECT_ROW) {
                    msg = Some(Msg::RowSelected(id));
                    break;
                }
                // The controls that are buttons. Each is its own attribute rather than a shared `data-action`, so a
                // click can only ever mean the one intent its attribute names.
                if let Some(key) = element.get_attribute(ATTRIBUTE_TAB) {
                    msg = Some(Msg::TabSelected(key));
                    break;
                }
                if let Some(delta) = element.get_attribute(ATTRIBUTE_PAGE) {
                    msg = delta.parse::<i64>().ok().map(Msg::PageChanged);
                    break;
                }
                if let Some(field) = element.get_attribute(ATTRIBUTE_CLEAR) {
                    msg = msg_for_input(&field, String::new());
                    break;
                }
                if element.is_same_node(Some(&root)) {
                    break;
                }
                node = element.parent_element();
            }
            if let Some(msg) = msg {
                deliver(msg);
            }
        }))
    };
    document.add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())?;
    listener.forget();

    // Typing. `input` fires on every character (and on paste and clear), which is what makes the list follow the field
    // as it is typed rather than after a commit the user never asks for.
    let on_input = {
        Closure::<dyn FnMut(Event)>::wrap(Box::new(move |event: Event| {
            let Some(root) = current_root().ok() else {
                return;
            };
            let Some(element) = inside_root(&root, &event) else {
                return;
            };
            let Some(field) = element.get_attribute(ATTRIBUTE_FIELD) else {
                return;
            };
            let Ok(input) = element.dyn_into::<HtmlInputElement>() else {
                return;
            };
            if let Some(msg) = msg_for_input(&field, input.value()) {
                deliver(msg);
            }
        }))
    };
    document.add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref())?;
    on_input.forget();

    // Choosing. A dropdown or a switch has committed by the time `change` fires, so the model is written from the
    // value the browser already holds rather than from a keystroke in progress.
    let on_change = {
        Closure::<dyn FnMut(Event)>::wrap(Box::new(move |event: Event| {
            let Some(root) = current_root().ok() else {
                return;
            };
            let Some(element) = inside_root(&root, &event) else {
                return;
            };
            if let Some(msg) = msg_for_change(&element) {
                deliver(msg);
            }
        }))
    };
    document.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())?;
    on_change.forget();

    Ok(())
}

/// Start the application in the container the PAGE owns, handed over by reference.
///
/// WHY THIS EXISTS, AND WHY IT IS THE ONE THE PAGES CALL. `#rust-ui` is an id, and an id is not an identity: during a
/// client-side navigation Next renders the new route while the old one is still in the document, so two containers can
/// carry that id at once and a lookup answers with the FIRST one in document order — the page the user is leaving. The
/// module would then mount the new screen into a container that was about to be removed, and the container the user
/// could actually see stayed empty. Handing over the node React owns removes the guess: there is no lookup left to get
/// wrong, and the page may hold the only container that exists.
#[wasm_bindgen]
pub fn start_in(root: HtmlElement) {
    if let Err(error) = start_at(&root) {
        web_sys::console::error_1(&error);
    }
}

fn start_at(root: &Element) -> Result<(), JsValue> {
    let document = document()?;
    adopt_actor(&document);
    // THE CONTAINER IS REMEMBERED HERE, because the listeners resolve it per event and must find the one that is live.
    ROOT.with(|slot| *slot.borrow_mut() = Some(root.clone()));

    let attribute = |name: &str| {
        root.get_attribute(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };

    // THE PAGE SAYS WHICH APPLICATION IT IS, because inference from the screen alone cannot work: the public app owns
    // its own URL routing and is mounted without naming a screen at all.
    match attribute(ATTR_APP).as_deref() {
        Some("site") => {
            crate::yew_app::mount_in(root.clone());
            Ok(())
        }
        // THE ERROR BOUNDARY'S PAGE. It is not a route and not a screen: it is rendered at whatever URL failed, so the
        // page names the application it wants and this mounts the one view no link can reach.
        Some("site-error") => {
            crate::yew_app::mount_error_in(root.clone());
            Ok(())
        }
        Some("portal") => {
            let Some(key) = attribute(ATTR_SCREEN) else {
                return Ok(());
            };
            let scope = attribute(ATTR_SCOPE).unwrap_or_default();
            // ONE PORTAL RENDERER, AND IT IS THE YEW APP. The branch that used to hand a screen without a Yew component
            // to the string renderer is gone: those screens render their existing body as markup INSIDE the Yew chrome
            // (`yew_portal::StringBody`), so there is one application, one chrome and one owner of this container
            // instead of two renderers racing over it.
            crate::yew_portal::mount_in(root.clone(), &key, &scope)
        }
        // No declaration: this page has no Rust UI. Not an error — the module loads on pages that never mount it.
        _ => Ok(()),
    }
}

/// The container the module mounts into, when the page does not name one.
/// The screen this container serves. Set by the page; read here, never passed as an argument.
const ATTR_SCREEN: &str = "data-rust-screen";
/// The record key, for a screen whose path carries one. Absent on list screens.
const ATTR_SCOPE: &str = "data-rust-scope";
/// Which application this container wants: `site`, `portal`, or `site-error` for the error boundary.
const ATTR_APP: &str = "data-rust-app";

/// The element the page writes the actor's projection into, as JSON. The portal layout renders it; the nav reads it.
const ACTOR_ID: &str = "rust-actor";

/// Adopt the actor projection the page wrote, so the nav can decide what to show.
///
/// CALLED ON EVERY MOUNT PATH — the portal app and the string host both render a rail, and the string host does not go
/// through `start`, so this cannot live only there. A missing element is normal (the public site has no actor) and
/// leaves the previous value alone.
fn adopt_actor(document: &Document) {
    let Some(element) = document.get_element_by_id(ACTOR_ID) else {
        return;
    };
    let json = element.text_content().unwrap_or_default();
    crate::navigation::set_actor(crate::navigation::actor_from_json(&json));
}
