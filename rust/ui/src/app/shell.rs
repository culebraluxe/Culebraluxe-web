//! THE MASTER SHELL — one Yew application, one router, for every URL: the public site and the portal.
//!
//! The URL is resolved against the registry; the entry decides the frame (site or portal chrome) and the content (a
//! `Screen` in its `ScreenHost`, or — until it is ported — the old loop's screen). Moving between screens of one area is
//! in-app: no page load, no fresh boot.
//!
//! WHAT NEXT STILL DOES: serves the document (every page file renders `<RustUi />`), guards `/portal` on the server,
//! and owns Auth.js. It no longer decides which screen shows: this router does, from the URL.

use std::cell::RefCell;

use yew::prelude::*;
use yew::AppHandle;
use yew_router::prelude::*;

use crate::app::chrome::frame;
use crate::app::registry::{self, Area, Kind};
use crate::app::screen::{parse_query, ScreenCtx};
use crate::model::Surface;

thread_local! {
    /// The one application on this document, so a re-mount destroys the previous one instead of leaving it alive.
    static MOUNTED: RefCell<Option<AppHandle<MasterApp>>> = const { RefCell::new(None) };
}

/// Mount the application into the container the page owns. `error` is the error boundary's page, which renders at the
/// URL that FAILED and so must not resolve that URL to a screen.
pub fn mount_in(root: web_sys::Element, error: bool) {
    console_error_panic_hook::set_once();
    // The old loop's markup screens are driven by document listeners until they are ported.
    crate::shell::listen();
    MOUNTED.with(|slot| {
        if let Some(previous) = slot.borrow_mut().take() {
            previous.destroy();
        }
    });
    let app = yew::Renderer::<MasterApp>::with_root_and_props(root, MasterProps { error }).render();
    MOUNTED.with(|slot| *slot.borrow_mut() = Some(app));
}

#[derive(Properties, PartialEq)]
pub struct MasterProps {
    pub error: bool,
}

#[function_component(MasterApp)]
pub fn master_app(props: &MasterProps) -> Html {
    html! {
        <BrowserRouter>
            <Frame error={props.error} />
        </BrowserRouter>
    }
}

#[function_component(Frame)]
fn frame_component(props: &MasterProps) -> Html {
    let location = use_location();
    let path = location
        .as_ref()
        .map(|location| location.path().to_string())
        .unwrap_or_else(|| "/".into());
    let query = location
        .as_ref()
        .map(|location| location.query_str().to_string())
        .unwrap_or_default();

    // THE GRANTS ARE READ ONCE PER PORTAL VISIT, here, and handed to every screen through `ScreenCtx::can`. No screen
    // reads them itself. A failed read leaves them `None`, which offers nothing: the fail-closed direction.
    let grants = use_state(|| None::<crate::model::PortalEntitlements>);
    {
        let grants = grants.clone();
        let portal = !props.error && registry::area_of(&path) == Area::Portal;
        use_effect_with(portal, move |portal| {
            if *portal && grants.is_none() {
                yew::platform::spawn_local(async move {
                    if let Ok(read) = crate::app::exec::fetch(crate::app::api::Entitlements).await {
                        grants.set(Some(read));
                    }
                });
            }
        });
    }

    // A new screen starts at the top of the page, as a page load would.
    use_effect_with(path.clone(), |_| {
        if let Some(window) = web_sys::window() {
            window.scroll_to_with_x_and_y(0.0, 0.0);
        }
    });

    if props.error {
        return frame(
            Area::Site,
            Surface::Site,
            html! { <crate::app::site::ErrorView /> },
        );
    }

    let Some((entry, params)) = registry::resolve(&path) else {
        let area = registry::area_of(&path);
        return frame(area, Surface::Core, not_found(area));
    };

    let ctx = ScreenCtx {
        actor: crate::navigation::actor(),
        id: params.first().map(|(_, value)| value.clone()),
        query: parse_query(&query),
        path: path.clone(),
        grants: (*grants).clone(),
    };
    let content = match entry.kind {
        // Keyed by screen: another screen is a fresh host; the same screen with a new record re-inits in place.
        Kind::Screen(mount) => html! {
            <div key={entry.key} class="min-w-0" data-screen-key={entry.key}>{ mount(ctx) }</div>
        },
        Kind::LegacyPortal(screen_key) => html! {
            <div key={format!("legacy:{path}")} class="min-w-0" data-screen-key={entry.key}>
                <LegacyPortal screen_key={screen_key} scope={ctx.id.clone()} />
            </div>
        },
        Kind::External => html! { <DocumentLoad href={path.clone()} /> },
    };
    frame(entry.area(), entry.surface, content)
}

#[derive(Properties, PartialEq)]
struct LegacyPortalProps {
    screen_key: &'static str,
    scope: Option<String>,
}

/// A portal screen still on the old loop, hosted in the master frame until it is ported. Temporary by construction:
/// the registry's `legacy_count_only_goes_down` ledger counts every use.
#[function_component(LegacyPortal)]
fn legacy_portal(props: &LegacyPortalProps) -> Html {
    match crate::model::screen(props.screen_key) {
        Some(screen) => {
            html! { <crate::yew_portal::PortalApp screen={screen} scope={props.scope.clone()} /> }
        }
        None => {
            html! { <p role="alert">{ format!("'{}' is not a known screen.", props.screen_key) }</p> }
        }
    }
}

#[derive(Properties, PartialEq)]
struct DocumentLoadProps {
    href: String,
}

/// A path Next renders itself (Auth.js, the React Forms editor) reached by in-app navigation: load the document.
#[function_component(DocumentLoad)]
fn document_load(props: &DocumentLoadProps) -> Html {
    let href = props.href.clone();
    use_effect_with(href, |href| {
        if let Some(window) = web_sys::window() {
            let _ = window.location().replace(href);
        }
    });
    Html::default()
}

fn not_found(area: Area) -> Html {
    let (back, label) = match area {
        Area::Site => ("/", "Return to the home page"),
        Area::Portal => ("/portal/dashboard", "Return to the cockpit"),
    };
    html! {
        <section class="px-6 py-32 md:px-12" data-screen-key="not-found">
            <div class="mx-auto max-w-3xl">
                <p class="mb-5 text-xs font-light uppercase tracking-[0.34em] text-accent">{"404"}</p>
                <h1 class="font-serif text-4xl font-light leading-[1.05] text-foreground md:text-5xl">
                    {"That page is not here."}
                </h1>
                <p class="mt-6 max-w-xl text-sm font-light leading-relaxed text-muted-foreground">
                    {"The address you followed does not match a page of this site."}
                </p>
                <crate::app::chrome::AppLink href={back} classes={classes!("mt-8", "inline-flex", "text-xs", "font-light", "uppercase", "tracking-[0.2em]", "text-accent")}>
                    { label }
                </crate::app::chrome::AppLink>
            </div>
        </section>
    }
}
