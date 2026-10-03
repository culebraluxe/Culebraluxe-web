//! Reading the browser in the user's own gesture: event values, chosen files, the file picker, confirm, a UUID.

#[allow(unused_imports)]
use super::*;

// ---- Reading the browser in the user's own gesture ------------------------------------------------------------------
//
// Screens never name `web_sys`. What a view needs from an event — a field's value, a checkbox, the chosen files — and
// the three things that must happen synchronously inside the user's click (Safari refuses a file picker or a dialog
// opened later) are here, beside the executor, so `app/screens/` has no browser type in it at all.

/// A file the operator chose. Screens hold these in their model and hand them to `Cmd::upload` / `Cmd::post_form`.
pub type File = web_sys::File;

use yew::TargetCast as _;

/// The text of the `<input>` an event came from.
pub fn input_value(event: &yew::Event) -> String {
    event
        .target_unchecked_into::<web_sys::HtmlInputElement>()
        .value()
}

/// The chosen option of the `<select>` an event came from.
pub fn select_value(event: &yew::Event) -> String {
    event
        .target_unchecked_into::<web_sys::HtmlSelectElement>()
        .value()
}

/// The text of the `<textarea>` an event came from.
pub fn textarea_value(event: &yew::Event) -> String {
    event
        .target_unchecked_into::<web_sys::HtmlTextAreaElement>()
        .value()
}

/// Whether the checkbox an event came from is ticked.
pub fn checked(event: &yew::Event) -> bool {
    event
        .target_unchecked_into::<web_sys::HtmlInputElement>()
        .checked()
}

/// The files chosen in the `<input type=file>` an event came from, in order. The input is then cleared, so choosing the
/// same file again (after a failure) is a new choice.
pub fn take_files(event: &yew::Event) -> Vec<File> {
    let input = event.target_unchecked_into::<web_sys::HtmlInputElement>();
    let files = input
        .files()
        .map(|list| {
            (0..list.length())
                .filter_map(|index| list.get(index))
                .collect()
        })
        .unwrap_or_default();
    input.set_value("");
    files
}

/// A file's MIME type, e.g. `image/jpeg`.
pub fn file_type(file: &File) -> String {
    file.type_()
}

/// A file's name as the operator's machine gave it.
pub fn file_name(file: &File) -> String {
    file.name()
}

/// Open the hidden file input with this id. Called from the click handler itself: a picker opened outside the user's
/// gesture is refused by Safari.
pub fn open_file_picker(id: &str) {
    if let Some(input) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(id))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        input.click();
    }
}

/// The browser's confirm dialog, answered synchronously inside the click that asked.
pub fn confirm(message: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.confirm_with_message(message).ok())
        .unwrap_or(false)
}

/// `crypto.randomUUID()`; empty when the browser has no Web Crypto (the caller's server rejects an empty id).
pub fn random_uuid() -> String {
    let Some(window) = web_sys::window() else {
        return String::new();
    };
    js_sys::Reflect::get(&window, &"crypto".into())
        .ok()
        .and_then(|crypto| {
            let function = js_sys::Reflect::get(&crypto, &"randomUUID".into()).ok()?;
            let function = function.dyn_into::<js_sys::Function>().ok()?;
            function.call0(&crypto).ok()?.as_string()
        })
        .unwrap_or_default()
}
