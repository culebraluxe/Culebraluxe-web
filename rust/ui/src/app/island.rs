//! `<Island>` — a vendor widget (SVAR Gantt, FullCalendar, a React gallery) inside a Yew screen, as a dumb widget.
//! The contract: docs/agent/UI-SCREEN-ARCHITECTURE.md §9.
//!
//! Yew owns the node. This component renders an empty `<div>` and tells the page's island host
//! (`components/rust-ui/island-host.tsx`) three things about it, in order: MOUNT (this node, this kind, these props, and
//! where to send events), UPDATE (new props), UNMOUNT. The host renders the widget into the node with a React portal —
//! inside Next's tree, so App Router context still works — and sends the widget's events back as JSON, which arrive
//! here as `on_event`. The widget reads no DOM outside its node and holds no application state; the screen does.
//!
//! THE HAND-OFF IS A QUEUE, so neither side has to start first: operations go onto `window.__culebraIslandOps`, and
//! the host drains it whenever it is told to (`window.__culebraIslandFlush`) or when it starts. No scanning, no
//! `MutationObserver`, no bridge buttons.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct IslandProps {
    /// Which widget (`ui-lab-gallery`, `project-gantt`, ...): a key in the host's renderer table.
    pub kind: AttrValue,
    /// The widget's props as JSON. A new value re-renders the widget; the same value does nothing.
    #[prop_or_else(|| AttrValue::from("null"))]
    pub props: AttrValue,
    /// The widget's events, as the JSON it sent.
    #[prop_or_default]
    pub on_event: Callback<serde_json::Value>,
    #[prop_or_default]
    pub class: Classes,
}

thread_local! {
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
}

/// Queue one operation for the host and ask it to drain the queue if it is running.
fn send(op: &js_sys::Object) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let queue_key = JsValue::from_str("__culebraIslandOps");
    let queue = js_sys::Reflect::get(&window, &queue_key)
        .ok()
        .and_then(|value| value.dyn_into::<js_sys::Array>().ok())
        .unwrap_or_else(|| {
            let queue = js_sys::Array::new();
            let _ = js_sys::Reflect::set(&window, &queue_key, &queue);
            queue
        });
    queue.push(op);
    if let Ok(flush) = js_sys::Reflect::get(&window, &JsValue::from_str("__culebraIslandFlush")) {
        if let Some(flush) = flush.dyn_ref::<js_sys::Function>() {
            let _ = flush.call0(&JsValue::NULL);
        }
    }
}

fn op(fields: &[(&str, &JsValue)]) -> js_sys::Object {
    let object = js_sys::Object::new();
    for (key, value) in fields {
        let _ = js_sys::Reflect::set(&object, &JsValue::from_str(key), value);
    }
    object
}

#[function_component]
pub fn Island(props: &IslandProps) -> Html {
    let node = use_node_ref();
    let id = *use_memo((), |_| {
        NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id + 1);
            id
        })
    });
    // The latest callback, so a re-render with a new closure is heard without remounting the widget.
    let listener = use_mut_ref(Callback::<serde_json::Value>::noop);
    *listener.borrow_mut() = props.on_event.clone();
    // The props the widget was mounted with, so the update effect below does not send them twice.
    let sent = use_mut_ref(String::new);

    {
        let node = node.clone();
        let listener = listener.clone();
        let sent = sent.clone();
        let props_json = props.props.clone();
        use_effect_with(props.kind.clone(), move |kind| {
            let emit = Closure::<dyn Fn(String)>::new(move |json: String| {
                // A widget event that is not JSON is the widget's defect: said, not guessed at.
                let event = serde_json::from_str(&json).unwrap_or_else(
                    |error| serde_json::json!({ "type": "invalid", "error": error.to_string() }),
                );
                let callback = listener.borrow().clone();
                callback.emit(event);
            });
            let emit = Rc::new(RefCell::new(Some(emit)));
            if let Some(element) = node.cast::<web_sys::Element>() {
                *sent.borrow_mut() = props_json.to_string();
                let function: JsValue = emit
                    .borrow()
                    .as_ref()
                    .map(|closure| closure.as_ref().clone())
                    .unwrap_or(JsValue::UNDEFINED);
                send(&op(&[
                    ("op", &JsValue::from_str("mount")),
                    ("id", &JsValue::from(id)),
                    ("node", element.as_ref()),
                    ("kind", &JsValue::from_str(kind)),
                    ("props", &JsValue::from_str(&props_json)),
                    ("emit", &function),
                ]));
            }
            move || {
                send(&op(&[
                    ("op", &JsValue::from_str("unmount")),
                    ("id", &JsValue::from(id)),
                ]));
                // The host forgets the widget before any further event could reach this closure.
                emit.borrow_mut().take();
            }
        });
    }
    {
        let sent = sent.clone();
        use_effect_with(props.props.clone(), move |props_json| {
            if *sent.borrow() != props_json.as_str() {
                *sent.borrow_mut() = props_json.to_string();
                send(&op(&[
                    ("op", &JsValue::from_str("update")),
                    ("id", &JsValue::from(id)),
                    ("props", &JsValue::from_str(props_json)),
                ]));
            }
        });
    }

    html! { <div ref={node} class={props.class.clone()} data-island={props.kind.clone()}></div> }
}
