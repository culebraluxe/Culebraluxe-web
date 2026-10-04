//! The Google property map: the JS-API map with the CL pin.
//!
//! Port of `lib/google-maps-loader.ts` + `components/property/google-property-map.tsx`.
//! The loader injects the Maps script once (id `culebraluxe-google-maps-js`,
//! callback `__culebraLuxeGoogleMapsReady`), then mounts a styled map with the
//! bespoke pin. The pin is the design, so its SVG lives here verbatim: navy
//! teardrop, gold ring, gold CL monogram, tip on the coordinate.
//!
//! The container is looked up by id when the command runs. The host executes
//! commands before the re-render commits, so the element may not exist yet —
//! the command waits for it briefly rather than failing a race it created.

use js_sys::{Object, Reflect};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use yew::platform::spawn_local;
use yew::Callback;

use crate::app::cmd::ApiError;

const SCRIPT_ID: &str = "culebraluxe-google-maps-js";
const CALLBACK: &str = "__culebraLuxeGoogleMapsReady";
// AdvancedMarkerElement requires a mapId on the map instance. This mirrors
// the established Google demo map id used by the dev spike. A dedicated
// production map id can be swapped in here later without touching marker
// semantics.
const PROPERTY_MAP_ID: &str = "DEMO_MAP_ID";

const PIN_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="44" height="52" viewBox="0 0 44 52"><path fill="#030f23" stroke="#c6a15b" stroke-width="2" d="M22 1C10.4 1 1 10.4 1 22c0 15.1 21 29 21 29s21-13.9 21-29C43 10.4 33.6 1 22 1Z"/><circle cx="22" cy="21" r="12" fill="#030f23" stroke="#c6a15b" stroke-width="1"/><text x="22" y="25" text-anchor="middle" fill="#c6a15b" font-family="Arial, sans-serif" font-size="10" font-weight="700">CL</text></svg>"##;

pub fn init_map<Msg: 'static>(
    key: String,
    lat: f64,
    lng: f64,
    title: String,
    container_id: String,
    reply: Box<dyn FnOnce(Result<(), ApiError>) -> Msg>,
    deliver: Callback<Msg>,
) {
    spawn_local(async move {
        let result = init_map_async(&key, lat, lng, &title, &container_id).await;
        deliver.emit(reply(result));
    });
}

fn network(message: &str) -> ApiError {
    ApiError::network(message.to_owned())
}

fn maps_namespace() -> Result<JsValue, ApiError> {
    let window = web_sys::window().ok_or_else(|| network("The browser has no window."))?;
    let google = Reflect::get(&window, &JsValue::from_str("google"))
        .map_err(|_| network("Google Maps did not load."))?;
    if google.is_undefined() {
        return Err(network("Google Maps did not load."));
    }
    let maps = Reflect::get(&google, &JsValue::from_str("maps"))
        .map_err(|_| network("Google Maps did not load."))?;
    let marker = Reflect::get(&maps, &JsValue::from_str("marker")).ok();
    if marker.is_none() || marker.unwrap().is_undefined() {
        return Err(network("Google Maps marker library did not load."));
    }
    Ok(maps)
}

async fn init_map_async(
    key: &str,
    lat: f64,
    lng: f64,
    title: &str,
    container_id: &str,
) -> Result<(), ApiError> {
    if maps_namespace().is_err() {
        load_script(key).await?;
    }
    let maps = maps_namespace()?;
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| network("The browser has no document."))?;
    // The host runs commands before the re-render commits, so the container
    // the view just drew may not exist yet. Wait briefly; two seconds of
    // 100ms polls, then a real failure rather than a race.
    let mut element = None;
    for _ in 0..20 {
        if let Some(found) = document.get_element_by_id(container_id) {
            element = Some(found);
            break;
        }
        yew::platform::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let element = element.ok_or_else(|| network("The map container was not drawn."))?;

    let center = Object::new();
    Reflect::set(&center, &JsValue::from_str("lat"), &JsValue::from_f64(lat))
        .map_err(|_| network("The map coordinate could not be set."))?;
    Reflect::set(&center, &JsValue::from_str("lng"), &JsValue::from_f64(lng))
        .map_err(|_| network("The map coordinate could not be set."))?;

    let map_type_ids = js_sys::Array::new();
    for id in ["ROADMAP", "SATELLITE", "HYBRID"] {
        let value = Reflect::get(
            &Reflect::get(&maps, &JsValue::from_str("MapTypeId"))
                .map_err(|_| network("Google Maps did not load."))?,
            &JsValue::from_str(id),
        )
        .map_err(|_| network("Google Maps did not load."))?;
        map_type_ids.push(&value);
    }
    let control_options = Object::new();
    let dropdown = Reflect::get(
        &Reflect::get(&maps, &JsValue::from_str("MapTypeControlStyle"))
            .map_err(|_| network("Google Maps did not load."))?,
        &JsValue::from_str("DROPDOWN_MENU"),
    )
    .map_err(|_| network("Google Maps did not load."))?;
    Reflect::set(
        &control_options,
        &JsValue::from_str("style"),
        &dropdown,
    )
    .map_err(|_| network("The map controls could not be set."))?;
    Reflect::set(
        &control_options,
        &JsValue::from_str("mapTypeIds"),
        &map_type_ids,
    )
    .map_err(|_| network("The map controls could not be set."))?;

    let options = Object::new();
    Reflect::set(&options, &JsValue::from_str("center"), &center)
        .map_err(|_| network("The map could not be configured."))?;
    Reflect::set(&options, &JsValue::from_str("zoom"), &JsValue::from_f64(14.0))
        .map_err(|_| network("The map could not be configured."))?;
    Reflect::set(
        &options,
        &JsValue::from_str("mapId"),
        &JsValue::from_str(PROPERTY_MAP_ID),
    )
    .map_err(|_| network("The map could not be configured."))?;
    Reflect::set(
        &options,
        &JsValue::from_str("mapTypeControlOptions"),
        &control_options,
    )
    .map_err(|_| network("The map could not be configured."))?;
    for (name, value) in [
        ("mapTypeControl", true),
        ("zoomControl", true),
        ("cameraControl", true),
        ("streetViewControl", false),
        ("fullscreenControl", true),
        ("clickableIcons", false),
    ] {
        Reflect::set(
            &options,
            &JsValue::from_str(name),
            &JsValue::from_bool(value),
        )
        .map_err(|_| network("The map could not be configured."))?;
    }
    Reflect::set(
        &options,
        &JsValue::from_str("gestureHandling"),
        &JsValue::from_str("auto"),
    )
    .map_err(|_| network("The map could not be configured."))?;

    let map_ctor: js_sys::Function = Reflect::get(&maps, &JsValue::from_str("Map"))
        .map_err(|_| network("Google Maps did not load."))?
        .dyn_into()
        .map_err(|_| network("Google Maps did not load."))?;
    let map = Reflect::construct(
        &map_ctor,
        &js_sys::Array::of2(&element.into(), &options.into()),
    )
    .map_err(|_| network("The map could not be created."))?;

    // The SVG is 44x52 and its pin tip is at the bottom edge; translateY(-26px)
    // lifts it half its height so the tip rests exactly on the coordinate.
    let content: web_sys::HtmlElement = document
        .create_element("div")
        .map_err(|_| network("The map pin could not be drawn."))?
        .dyn_into()
        .map_err(|_| network("The map pin could not be drawn."))?;
    content
        .set_attribute("style", "transform: translateY(-26px);")
        .map_err(|_| network("The map pin could not be drawn."))?;
    content.set_inner_html(PIN_SVG);

    let marker_ns = Reflect::get(&maps, &JsValue::from_str("marker"))
        .map_err(|_| network("Google Maps marker library did not load."))?;
    let marker_ctor: js_sys::Function =
        Reflect::get(&marker_ns, &JsValue::from_str("AdvancedMarkerElement"))
            .map_err(|_| network("Google Maps marker library did not load."))?
            .dyn_into()
            .map_err(|_| network("Google Maps marker library did not load."))?;
    let marker_options = Object::new();
    Reflect::set(&marker_options, &JsValue::from_str("map"), &map)
        .map_err(|_| network("The map pin could not be placed."))?;
    Reflect::set(&marker_options, &JsValue::from_str("position"), &center)
        .map_err(|_| network("The map pin could not be placed."))?;
    Reflect::set(
        &marker_options,
        &JsValue::from_str("title"),
        &JsValue::from_str(&format!("{title} property location")),
    )
    .map_err(|_| network("The map pin could not be placed."))?;
    Reflect::set(
        &marker_options,
        &JsValue::from_str("content"),
        &content,
    )
    .map_err(|_| network("The map pin could not be placed."))?;
    Reflect::set(
        &marker_options,
        &JsValue::from_str("gmpClickable"),
        &JsValue::from_bool(true),
    )
    .map_err(|_| network("The map pin could not be placed."))?;
    Reflect::construct(
        &marker_ctor,
        &js_sys::Array::of1(&marker_options.into()),
    )
    .map_err(|_| network("The map pin could not be placed."))?;
    Ok(())
}

async fn load_script(key: &str) -> Result<(), ApiError> {
    let window = web_sys::window().ok_or_else(|| network("The browser has no window."))?;
    let document = window
        .document()
        .ok_or_else(|| network("The browser has no document."))?;
    if document.get_element_by_id(SCRIPT_ID).is_some() {
        // Another map beat us here: its load (or failure) decides.
        return wait_for_maps().await;
    }
    let script: web_sys::HtmlScriptElement = document
        .create_element("script")
        .map_err(|_| network("The map script could not be created."))?
        .dyn_into()
        .map_err(|_| network("The map script could not be created."))?;
    script.set_id(SCRIPT_ID);
    script.set_src(&format!(
        "https://maps.googleapis.com/maps/api/js?key={key}&loading=async&libraries=marker&callback={CALLBACK}"
    ));
    script.set_async(true);
    script.set_defer(true);

    let ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let fire = ready.clone();
    let closure = Closure::wrap(Box::new(move || {
        fire.store(true, std::sync::atomic::Ordering::Release);
    }) as Box<dyn Fn()>);
    Reflect::set(
        &window,
        &JsValue::from_str(CALLBACK),
        closure.as_ref().unchecked_ref(),
    )
    .map_err(|_| network("The map callback could not be installed."))?;
    document
        .head()
        .ok_or_else(|| network("The page has no head element."))?
        .append_child(&script)
        .map_err(|_| network("The map script could not be started."))?;

    for _ in 0..60 {
        if ready.load(std::sync::atomic::Ordering::Acquire) {
            closure.forget();
            return maps_namespace().map(|_| ());
        }
        yew::platform::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    // Thirty seconds with no callback: the key, the network, or the marker
    // library. A failure here is a state, not a throw — the page draws it.
    Err(network(
        "Google Maps did not answer. Check the key and the connection, then try again.",
    ))
}

async fn wait_for_maps() -> Result<(), ApiError> {
    for _ in 0..60 {
        if maps_namespace().is_ok() {
            return Ok(());
        }
        yew::platform::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Err(network(
        "Google Maps did not answer. Check the key and the connection, then try again.",
    ))
}
