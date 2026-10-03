//! The browser entry point: the page hands the application its container, and this mounts the one Yew app in it.
//!
//! Everything else that touches the DOM is in `app/exec.rs`. (This file used to also hold the document listeners that
//! drove the old loop's HTML-string screens through `data-*` attributes; that loop was deleted on 2026-09-28.)
#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement};

fn document() -> Result<Document, JsValue> {
    web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))?
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))
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
    let attribute = |name: &str| {
        root.get_attribute(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };

    // THE PAGE SAYS WHICH APPLICATION IT IS, because inference from the screen alone cannot work: the public app owns
    // its own URL routing and is mounted without naming a screen at all.
    // ONE APPLICATION. Site and portal pages both mount the master shell, whose router reads the URL
    // (app/shell.rs); `site-error` is the error boundary's page, which must not resolve the failed URL.
    match attribute(ATTR_APP).as_deref() {
        Some("site") | Some("portal") => {
            crate::app::shell::mount_in(root.clone(), false);
            Ok(())
        }
        Some("site-error") => {
            crate::app::shell::mount_in(root.clone(), true);
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Which application this container wants: `site`, `portal`, or `site-error` for the error boundary.
const ATTR_APP: &str = "data-rust-app";

/// The element the page writes the actor's projection into, as JSON. The portal layout renders it; the nav reads it.
const ACTOR_ID: &str = "rust-actor";

/// Adopt the actor projection the page wrote, so the nav can decide what to show.
///
/// A missing element is normal (the public site has no actor) and leaves the previous value alone.
fn adopt_actor(document: &Document) {
    let Some(element) = document.get_element_by_id(ACTOR_ID) else {
        return;
    };
    let json = element.text_content().unwrap_or_default();
    crate::navigation::set_actor(crate::navigation::actor_from_json(&json));
}
