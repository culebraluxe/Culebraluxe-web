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
/// A container the host mounts a third-party widget into. Rust renders the box; the widget owns what is inside it.
const ATTRIBUTE_ISLAND: &str = "data-island";

/// The DOM event the shell announces effects on, and the one name the TypeScript host has to agree with.
const EFFECT_EVENT: &str = "rust-ui:effects";

thread_local! {
    static PROGRAM: RefCell<Option<Rc<RefCell<Program>>>> = const { RefCell::new(None) };
    /// Whether the document listeners are already installed.
    ///
    /// THE HOST CALLS `mount` ON EVERY EFFECT RUN, and React runs effects twice in development. Installing the listeners
    /// each time would mean two of each on the document, so a single click would dispatch its intent twice — a double
    /// navigation, or a row opened twice.
    ///
    /// THEY ARE ON THE DOCUMENT AND THEY RESOLVE WHAT THEY NEED PER EVENT: the current program out of `PROGRAM`, and the
    /// current container out of the DOM (`current_root`). That second half is what an earlier version got wrong: it
    /// captured the root element it was installed on, so once React replaced `<div id="rust-ui">` the listeners were
    /// attached to a detached node and the replacement root had none — the screen painted and every control was dead.
    static LISTENERS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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

/// What a paint has to replace.
///
/// SCREEN-LOCAL STATE MAY REPAINT THE SCREEN; IT MUST NOT DESTROY APPLICATION CHROME. The whole mount point used to be
/// replaced after every message, so a keystroke in the Buyers search field rebuilt the header and the footer — and with
/// them the mobile menu's `<details>` open state, which belongs to the DOM and cannot survive being thrown away. The
/// distinction is derived from the model (see `Program::chrome_signature`), not from a list of messages, so a message
/// added later cannot silently invalidate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Repaint {
    /// The screen changed: the header's current destination moves with it, so the chrome is rewritten too.
    Chrome,
    /// Only the screen's own state changed: the chrome is left exactly as it is.
    Page,
}

/// Write the current model into the container. The single place the DOM is written, which is also why the focus
/// handling lives here and nowhere else.
fn paint(root: &HtmlElement, program: &Rc<RefCell<Program>>, repaint: Repaint) {
    let focus = capture_focus();
    // The page-only path falls back to the whole document when there is no chrome to keep — which is the state of the
    // mount point before the first render, and the reason a screen-local message arriving first cannot paint half a page.
    let repaint_target = match repaint {
        Repaint::Chrome => None,
        Repaint::Page => root.query_selector(&format!("#{}", crate::view::PAGE_ID)).ok().flatten(),
    };
    match repaint_target {
        Some(page) => page.set_inner_html(&program.borrow().page_html()),
        None => root.set_inner_html(&program.borrow().html()),
    }
    if let Some(focus) = focus {
        restore_focus(&focus);
    }
    // Last, because it is about the markup that was just written: any island container in it needs its widget.
    announce_islands(root);
}

/// The DOM event the shell announces islands on. The host mounts a third-party widget into each container this crate
/// renders, so Rust keeps owning the layout and the widget owns everything inside its own box.
const ISLAND_EVENT: &str = "rust-ui:islands";

/// What the host is told about each island after a paint: which widget goes in which container.
///
/// WHY AFTER EVERY PAINT: `set_inner_html` destroys the container a widget was mounted into, so the container is new
/// every time and the widget has to be mounted again. The host decides whether to remount by looking at the container -
/// a container that already has a child is one whose widget survived - so this can be announced unconditionally without
/// remounting anything that is still alive.
fn announce_islands(root: &HtmlElement) {
    let Ok(document) = document() else { return };
    let Ok(nodes) = root.query_selector_all(&format!("[{ATTRIBUTE_ISLAND}]")) else {
        return;
    };
    let mut islands: Vec<String> = Vec::new();
    for index in 0..nodes.length() {
        let Some(node) = nodes.item(index) else { continue };
        let Ok(element) = node.dyn_into::<Element>() else {
            continue;
        };
        let Some(name) = element.get_attribute(ATTRIBUTE_ISLAND) else {
            continue;
        };
        islands.push(name);
    }
    if islands.is_empty() {
        return;
    }
    // The names are values this crate rendered, so the JSON is built here rather than reached for through `js_sys`.
    let payload = format!(
        "[{}]",
        islands
            .iter()
            .map(|name| format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(",")
    );
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(&payload));
    init.set_bubbles(true);
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(ISLAND_EVENT, &init) {
        let _ = document.dispatch_event(&event);
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

/// The program a message belongs to, right now.
///
/// THE LISTENERS MUST READ THIS RATHER THAN CAPTURE A PROGRAM. The host calls `mount` more than once (React runs effects
/// twice in development), and every mount replaces `PROGRAM`. A listener created by an earlier mount that dispatches
/// into the program it captured would repaint from stale state while `rows_loaded` — which reads `PROGRAM` — wrote into
/// the current one: two programs driving one container, which presents as a screen whose clicks do nothing.
fn current_program() -> Option<Rc<RefCell<Program>>> {
    PROGRAM.with(|slot| slot.borrow().clone())
}

/// Apply an intent to the current program. `None` when nothing is mounted, which is not an error worth raising: a click
/// that arrives after the host unmounted has nowhere to go.
fn dispatch_current(root: &HtmlElement, msg: Msg) -> Option<String> {
    let program = current_program()?;
    Some(dispatch(root, &program, msg))
}

fn dispatch(root: &HtmlElement, program: &Rc<RefCell<Program>>, msg: Msg) -> String {
    // What the chrome is a function of, before the message. Compared after it, so "does the header need rewriting?"
    // is answered by the model rather than by a guess about which messages navigate.
    let chrome_before = program.borrow().chrome_signature();
    let effects = program.borrow_mut().dispatch(msg);
    let repaint = if program.borrow().chrome_signature() == chrome_before {
        Repaint::Page
    } else {
        Repaint::Chrome
    };
    paint(root, program, repaint);
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

/// The `#rust-ui` container as it exists RIGHT NOW.
///
/// RESOLVED PER EVENT, NEVER CAPTURED. The listeners are installed once and used to capture the root element they were
/// installed on — so when React or Next replaced `<div id="rust-ui">`, every listener was left attached to a detached
/// node, and the next mount saw `LISTENERS_INSTALLED == true` and installed nothing on the replacement. The screen
/// painted, the controls were dead, and nothing said why. Looking the root up at event time is what makes a replaced
/// container work without a teardown protocol.
fn current_root() -> Result<HtmlElement, JsValue> {
    container(&mount_id())
}

/// Whether an event's target is inside the current root.
///
/// The listeners are on the document, so they see clicks on the whole page: the header's own links, a third-party
/// widget, anything. An intent-bearing attribute found outside the Rust UI is not this crate's to act on, and acting on
/// it would let one thing on the page drive another.
fn inside_root(root: &HtmlElement, event: &Event) -> Option<Element> {
    let target = event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())?;
    root.contains(Some(&target)).then_some(target)
}
#[wasm_bindgen]
pub fn island_event_name() -> String {
    ISLAND_EVENT.to_string()
}

/// Mount the program into `element_id`, opening `start` — a screen key such as `site-home` or `dashboard` — and return
/// the effects the host must run, as a JSON array.
///
/// The starting screen comes from the host because the host owns the URL. An unknown key is refused rather than
/// quietly opening the first screen: a page that renders the wrong screen without saying so is a bug that gets
/// debugged twice.
#[wasm_bindgen]
pub fn mount(element_id: &str, start: &str, generation: u64) -> Result<String, JsValue> {
    console_error_panic_hook::set_once();
    let start = screen(start)
        .ok_or_else(|| JsValue::from_str(&format!("ui: '{start}' is not a known screen")))?;
    let root = container(element_id)?;
    let program = Rc::new(RefCell::new(Program::new()));
    PROGRAM.with(|slot| *slot.borrow_mut() = Some(program.clone()));

    let already_listening = LISTENERS_INSTALLED.with(|flag| flag.replace(true));
    if already_listening {
        // The listeners are on the document and resolve the current program and the current container per event, so
        // mounting again only means painting the screen this host asked for into the container that exists now.
        return Ok(dispatch(
            &root,
            &program,
            Msg::Mount {
                screen: start,
                generation,
            },
        ));
    }

    let document = document()?;
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
                dispatch_current(&root, msg);
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
                dispatch_current(&root, msg);
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
                dispatch_current(&root, msg);
            }
        }))
    };
    document.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())?;
    on_change.forget();

    Ok(dispatch(
        &root,
        &program,
        Msg::Mount {
            screen: start,
            generation,
        },
    ))
}

/// The typed bridge: the host fetched the rows from an application route, and this is how they land.
///
/// `screen` AND `generation` ARE PART OF THE ANSWER. The host says which screen it fetched for and which mount asked;
/// the reducer refuses the payload if either has moved on (`update::owns`). Without them a response for one screen can
/// land while another is mounted — the browser shows the page it was told to and the model holds a different one.
///
/// The host owns the network on purpose. It holds the session; this module holds no credential, so a compromised view
/// layer cannot be talked into fetching somewhere else.
#[wasm_bindgen]
pub fn rows_loaded(screen: &str, generation: u64, payload: &str) -> Result<(), JsValue> {
    PROGRAM.with(|slot| {
        let program = slot.borrow().clone();
        match program {
            Some(program) => {
                let root = current_root()?;
                dispatch(&root, &program, Msg::rows_loaded_json(screen, generation, payload));
                Ok(())
            }
            None => Err(JsValue::from_str(
                "ui: mount was not called before rows_loaded",
            )),
        }
    })
}

/// The same bridge for a page: the host fetched its blocks from an application route, and it names the screen and the
/// mount they were fetched for so the reducer can refuse an answer whose screen has moved on.
#[wasm_bindgen]
pub fn page_loaded(screen: &str, generation: u64, payload: &str) -> Result<(), JsValue> {
    PROGRAM.with(|slot| {
        let program = slot.borrow().clone();
        match program {
            Some(program) => {
                let root = current_root()?;
                dispatch(&root, &program, Msg::page_loaded_json(screen, generation, payload));
                Ok(())
            }
            None => Err(JsValue::from_str(
                "ui: mount was not called before page_loaded",
            )),
        }
    })
}

/// The mount container id, so the host and the shell cannot disagree about it in silence.
#[wasm_bindgen]
pub fn mount_id() -> String {
    "rust-ui".to_string()
}
