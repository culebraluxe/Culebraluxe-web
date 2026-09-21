//! The browser shell — the only place in this crate that knows a DOM exists.
//!
//! It does three things and nothing else: mount the program into a container element, turn clicks into `Msg`s by
//! reading `data-` attributes off the nearest ancestor, and write `Program::html()` back into that container.
//!
//! THREE DECISIONS WORTH KNOWING, all of them the kind that bite later if left implicit:
//!
//! * **The click listener is leaked on purpose.** `Closure::forget` keeps it alive for the life of the page. The
//!   alternative is a `static` holding it, which is the same lifetime with more ceremony. Dropping the closure is the
//!   classic way a Rust WASM UI works once and then goes deaf.
//! * **The program lives in a thread-local.** WASM is single-threaded here, `Rc<RefCell<_>>` is not `Sync`, and
//!   passing an element id through every call would be noise. One program per page is the honest shape.
//! * **`mount` returns the effects as JSON**, so the TypeScript host knows what to fetch. The shell never performs a
//!   request: the browser calls the application's own Next routes, which hold the session and the server-side
//!   credentials. No internal key exists anywhere in this module.
//!
//! RENDERING REPLACES `innerHTML` for now. Correct for a screen made entirely of Rust-rendered markup, and exactly
//! what has to change for a third-party island — a widget's container must survive a re-render. That is why the
//! islands come last and the write is isolated in `paint`.
#![cfg(feature = "wasm")]

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, Event, HtmlElement, HtmlInputElement, HtmlSelectElement, MouseEvent, Window};

use crate::{home, screen, Msg, Program, Surface};

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

/// The DOM event the shell announces effects on, and the one name the TypeScript host has to agree with.
const EFFECT_EVENT: &str = "rust-ui:effects";

thread_local! {
    static PROGRAM: RefCell<Option<Rc<RefCell<Program>>>> = const { RefCell::new(None) };
}

fn window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("ui: no window"))
}

fn document() -> Result<Document, JsValue> {
    window()?
        .document()
        .ok_or_else(|| JsValue::from_str("ui: no document"))
}

fn container(element_id: &str) -> Result<HtmlElement, JsValue> {
    document()?
        .get_element_by_id(element_id)
        .ok_or_else(|| JsValue::from_str(&format!("ui: #{element_id} is not in the document")))?
        .dyn_into::<HtmlElement>()
        .map_err(|_| JsValue::from_str("ui: mount target is not an HTMLElement"))
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

/// Write the current model into the container. The single place the DOM is written, which is also why the focus
/// handling lives here and nowhere else.
fn paint(root: &HtmlElement, program: &Rc<RefCell<Program>>) {
    let focus = capture_focus();
    root.set_inner_html(&program.borrow().html());
    if let Some(focus) = focus {
        restore_focus(&focus);
    }
}

/// The message a named input field means. An unknown field is ignored: a control that dispatches nothing is a bug to
/// see, where a control that dispatches the wrong message is a bug to hunt.
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
        _ => None,
    }
}

/// Apply one intent, paint, and hand the caller the effects it must perform.
fn dispatch(root: &HtmlElement, program: &Rc<RefCell<Program>>, msg: Msg) -> String {
    let effects = program.borrow_mut().dispatch(msg);
    paint(root, program);
    let payload = serde_json::to_string(&effects).unwrap_or_else(|_| "[]".to_string());
    publish(&payload);
    payload
}

/// Effects that arise from a click are returned to nobody: the click happened inside this module's own listener, and
/// the value returned to JavaScript is the one from `mount`, long since consumed. So the shell announces them on the
/// document instead, and the host listens.
///
/// A DOM event rather than a registered JS callback, deliberately: it keeps the network on the host's side of the
/// boundary (this module still holds no credential and performs no request), it needs no `Function` handle kept alive
/// across the WASM/JS boundary, and it is observable — you can see the protocol in the devtools event log.
///
/// Nothing is emitted when there is nothing to do. An event that always fires is an event the host learns to ignore.
fn publish(effects_json: &str) {
    if effects_json == "[]" {
        return;
    }
    let Ok(document) = document() else { return };
    // The detail is a JSON *string* on purpose: the host already parses JSON, and parsing it here would need `js_sys`
    // for a value nobody on this side reads.
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(effects_json));
    init.set_bubbles(true);
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(EFFECT_EVENT, &init) {
        // Dispatch is best effort: a failure to notify must not abort the state change the user already caused.
        let _ = document.dispatch_event(&event);
    }
}

/// The DOM event name the host listens for. Named here, next to the shell that emits it, because a typo in a string
/// that only the TypeScript side knows about is a bug that fails silently.
#[wasm_bindgen]
pub fn effect_event_name() -> String {
    EFFECT_EVENT.to_string()
}

/// Mount the program into `element_id`, opening `start` — a screen key such as `site-home` or `dashboard` — and return
/// the effects the host must run, as a JSON array.
///
/// The starting screen comes from the host because the host owns the URL. An unknown key is refused rather than
/// quietly opening the first screen: a page that renders the wrong screen without saying so is a bug that gets
/// debugged twice.
#[wasm_bindgen]
pub fn mount(element_id: &str, start: &str) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let start = screen(start)
        .ok_or_else(|| JsValue::from_str(&format!("ui: '{start}' is not a known screen")))?;
    let root = container(element_id)?;
    let program = Rc::new(RefCell::new(Program::new()));
    PROGRAM.with(|slot| *slot.borrow_mut() = Some(program.clone()));

    let listener = {
        let root = root.clone();
        let program = program.clone();
        Closure::<dyn FnMut(MouseEvent)>::wrap(Box::new(move |event: MouseEvent| {
            // Walk up from the click target to the mount root, taking the first intent-bearing ancestor. A click on
            // a child of a row must select that row, and a widget's own markup must not have to know about this.
            let mut node = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Element>().ok());
            let mut msg = None;
            while let Some(element) = node {
                if let Some(key) = element.get_attribute(ATTRIBUTE_SURFACE) {
                    // A surface is a destination, not a screen: the switcher offers the surface's home, which is the
                    // one screen it can point at without guessing.
                    msg = Surface::from_key(&key).and_then(home).map(Msg::Navigate);
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
                dispatch(&root, &program, msg);
            }
        }))
    };
    root.add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())?;
    listener.forget();

    // Typing. `input` fires on every character (and on paste and clear), which is what makes the list follow the field
    // as it is typed rather than after a commit the user never asks for.
    let on_input = {
        let root = root.clone();
        let program = program.clone();
        Closure::<dyn FnMut(Event)>::wrap(Box::new(move |event: Event| {
            let Some(element) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            let Some(field) = element.get_attribute(ATTRIBUTE_FIELD) else {
                return;
            };
            let Ok(input) = element.dyn_into::<HtmlInputElement>() else {
                return;
            };
            if let Some(msg) = msg_for_input(&field, input.value()) {
                dispatch(&root, &program, msg);
            }
        }))
    };
    root.add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref())?;
    on_input.forget();

    // Choosing. A dropdown or a switch has committed by the time `change` fires, so the model is written from the
    // value the browser already holds rather than from a keystroke in progress.
    let on_change = {
        let root = root.clone();
        let program = program.clone();
        Closure::<dyn FnMut(Event)>::wrap(Box::new(move |event: Event| {
            let Some(element) = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            else {
                return;
            };
            if let Some(msg) = msg_for_change(&element) {
                dispatch(&root, &program, msg);
            }
        }))
    };
    root.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())?;
    on_change.forget();

    Ok(dispatch(&root, &program, Msg::ScreenOpened(start)))
}

/// The typed bridge: the host fetched the rows from an application route, and this is how they land.
///
/// The host owns the network on purpose. It holds the session; this module holds no credential, so a compromised view
/// layer cannot be talked into fetching somewhere else.
#[wasm_bindgen]
pub fn rows_loaded(payload: &str) -> Result<(), JsValue> {
    PROGRAM.with(|slot| {
        let program = slot.borrow().clone();
        match program {
            Some(program) => {
                let root = container(&mount_id())?;
                dispatch(&root, &program, Msg::rows_loaded_json(payload));
                Ok(())
            }
            None => Err(JsValue::from_str(
                "ui: mount was not called before rows_loaded",
            )),
        }
    })
}

/// The mount container id, so the host and the shell cannot disagree about it in silence.
#[wasm_bindgen]
pub fn mount_id() -> String {
    "rust-ui".to_string()
}
