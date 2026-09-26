//! Routing, and the switch that turns a URL into a screen.
//!
//! YEW ROUTER OWNS THE PATHS THIS APP SERVES, and no others. Today that is `/buyers`: the other public routes are still
//! rendered by the string renderers through the Next host, and a path this bundle does not own must not resolve here —
//! otherwise one URL would be claimed by two runtimes at once. Porting another screen is one variant plus one view.
//!
//! THE ROUTE CARRIES A SCREEN KEY, not a screen of its own. `Route::screen()` resolves through the same registry the MVI
//! model uses, so a URL cannot name a page the model does not know, and the payload a screen fetches is the payload its
//! registry entry says.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::model::{screen, Msg, Screen};
use crate::yew_views::about::About;
use crate::yew_views::account::Account;
use crate::yew_views::buyers::Buyers;
use crate::yew_views::contact::Contact;
use crate::yew_views::faq::Faq;
use crate::yew_views::favorites::Favorites;
use crate::yew_views::guide::Guide;
use crate::yew_views::home::Home;
use crate::yew_views::property_detail::PropertyDetail;
use crate::yew_views::sellers::Sellers;
use crate::yew_views::services::Services;

/// The public paths this app owns.
#[derive(Debug, Clone, PartialEq, Eq, Routable)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/buyers")]
    Buyers,
    #[at("/services")]
    Services,
    #[at("/about")]
    About,
    #[at("/sellers")]
    Sellers,
    #[at("/guide")]
    Guide,
    #[at("/faq")]
    Faq,
    #[at("/contact")]
    Contact,
    #[at("/favorites")]
    Favorites,
    #[at("/account")]
    Account,
    #[at("/properties/:slug")]
    Property { slug: String },
    #[at("/properties")]
    Properties,
    #[at("/privacy")]
    Privacy,
    #[at("/video")]
    Video,
    #[at("/whatsapp")]
    Whatsapp,
    #[at("/login")]
    Login,
    #[at("/login/recovery")]
    LoginRecovery,
    #[at("/login/unauthorized")]
    LoginUnauthorized,
    #[at("/auth/error")]
    AuthError,
    #[at("/review/:token/:page")]
    Review { token: String, page: String },
    /// The port's own preview harness: the public listings screen at a URL of its own, so the shell can be exercised
    /// without a reader of the real route. The router owns it because a URL must have one owner.
    #[at("/rust-preview")]
    RustPreview,
    #[at("/dev/google-map-test")]
    DevGoogleMap,
    #[at("/dev/apple-map-test")]
    DevAppleMap,
    /// A URL this app does not serve. It renders a message rather than a blank page, and the reader is offered the way
    /// back to the home page — the honest answer to a path that has no screen here.
    #[not_found]
    #[at("/404")]
    NotFound,
}

impl Route {
    /// The screen this URL serves, from the registry the model and the payload routes already share.
    pub fn screen(&self) -> Option<Screen> {
        match self {
            Route::Home => screen("site-home"),
            Route::Buyers => screen("site-buyers"),
            Route::Services => screen("site-services"),
            Route::About => screen("site-about"),
            Route::Sellers => screen("site-sellers"),
            Route::Guide => screen("site-guide"),
            Route::Faq => screen("site-faq"),
            Route::Contact => screen("site-contact"),
            Route::Favorites => screen("site-favorites"),
            Route::Account => screen("site-account"),
            Route::Property { .. } => screen("site-property-detail"),
            Route::Properties => screen("site-properties"),
            Route::Privacy => screen("site-privacy"),
            Route::Video => screen("site-video"),
            Route::Whatsapp => screen("site-whatsapp"),
            Route::Login => screen("login"),
            Route::LoginRecovery => screen("login-recovery"),
            Route::LoginUnauthorized => screen("login-unauthorized"),
            Route::AuthError => screen("auth-error"),
            Route::Review { .. } => screen("review"),
            // The preview harness shows the listings screen; the screen is the same one `/properties` serves, and it is
            // named here rather than invented so the payload it fetches is the payload that screen's registry entry says.
            Route::RustPreview => screen("site-properties"),
            Route::DevGoogleMap => screen("dev-google-map-test"),
            Route::DevAppleMap => screen("dev-apple-map-test"),
            Route::NotFound => None,
        }
    }

    /// The record key carried by a dynamic public route.
    pub fn scope(&self) -> Option<String> {
        match self {
            Route::Property { slug } => Some(slug.clone()),
            // An enquiry about one property carries its id in the query (`/contact?propertyId=...`), the link every
            // card and the detail page build. It scopes the page so the payload can name the property.
            Route::Contact => query_param("propertyId"),
            _ => None,
        }
    }
}

/// One value from the current page's query string, decoded. `None` when it is absent or empty.
pub fn query_param(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search.trim_start_matches('?').split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (key == name)
            .then(|| {
                js_sys::decode_uri_component(&value.replace('+', " "))
                    .ok()
                    .map(String::from)
            })
            .flatten()
            .filter(|value| !value.trim().is_empty())
    })
}

#[derive(Properties, PartialEq)]
pub struct ShellProps {
    /// The screen's state. Cloned into the route component that renders, because a Yew component owns what it is given.
    pub model: crate::model::Model,
    /// Every control's intent, dispatched into the one reducer.
    pub on_msg: Callback<Msg>,
}

/// The application: the public chrome, and the screen the current URL serves.
pub struct Shell;

impl Component for Shell {
    type Message = ();
    type Properties = ShellProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let model = props.model.clone();
        let on_msg = props.on_msg.clone();
        html! {
            <div class="flex min-h-screen flex-col bg-background text-foreground">
                <crate::yew_views::chrome::Header />
                <main class="min-w-0 flex-1">
                    <Switch<Route> render={Callback::from(move |route: Route| match route {
                        Route::Home => html! { <Home model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Buyers => html! { <Buyers model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Services => html! { <Services model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::About => html! { <About model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Sellers => html! { <Sellers model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Guide => html! { <Guide model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Faq => html! { <Faq model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Contact => html! { <Contact model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Favorites => html! { <Favorites model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Account => html! { <Account model={model.clone()} on_msg={on_msg.clone()} /> },
                        Route::Property { .. } => html! {
                            <PropertyDetail model={model.clone()} on_msg={on_msg.clone()} />
                        },
                        // THE PUBLIC SCREENS THAT HAVE NO COMPONENT OF THEIR OWN yet render the body the port already
                        // produces, inside this chrome and this router. Same bridge as the portal's, same reason: the
                        // URL, the header, the footer and the route must have ONE owner, and it is the Yew app.
                        Route::Properties
                        | Route::Privacy
                        | Route::Video
                        | Route::Whatsapp
                        | Route::Login
                        | Route::LoginRecovery
                        | Route::LoginUnauthorized
                        | Route::AuthError
                        | Route::Review { .. }
                        | Route::RustPreview
                        | Route::DevGoogleMap
                        | Route::DevAppleMap => html! { <SiteBody model={model.clone()} /> },
                        Route::NotFound => html! { <NotFound /> },
                    })} />
                </main>
                <crate::yew_views::chrome::Footer />
            </div>
        }
    }
}

/// A public screen whose body is the one `view::render_page` already produces.
///
/// THE SAME BRIDGE THE PORTAL USES, and it is not a second renderer: one Yew app owns the URL, the chrome and the page,
/// and a screen that has not been rewritten as a component renders the markup the port already had. `VNode::VRaw` is
/// diffed and updated by Yew like any other node, so the body follows the model on every message.
#[derive(Properties, PartialEq)]
pub struct SiteBodyProps {
    pub model: crate::model::Model,
}

#[function_component(SiteBody)]
fn site_body(props: &SiteBodyProps) -> Html {
    Html::from_html_unchecked(yew::AttrValue::from(crate::view::render_page(&props.model)))
}

/// A path this app does not own.
///
/// THE DESIGN IS THE ONE THE REACT PAGE HAD, ported rather than replaced: full-bleed, dark, one large 404, two ways out.
/// It is a Yew component and not a string body because it needs no model — there is nothing about a 404 to fetch — and it
/// renders inside this application's own header and footer, which is what retires the last React chrome in the
/// repository (`components/site-header.tsx` and `site-footer.tsx`, whose only user this page was).
///
/// IT IS A `<section>`, NOT A `<main>`: this application's shell already renders the `<main>`, and two nested ones is
/// invalid markup. The React page had to own the `<main>` because it owned the whole document.
#[function_component(NotFound)]
fn not_found() -> Html {
    html! {
        <section class="bg-foreground px-6 py-32 text-background md:px-12 md:py-44">
            <div class="mx-auto max-w-[1600px]">
                <p class="mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/60">{"CulebraLuxe"}</p>
                <h1 class="text-balance font-serif text-5xl font-light leading-[1.02] text-background md:text-7xl">
                    {"404"}
                </h1>
                <p class="mt-6 max-w-xl text-pretty text-base font-light leading-relaxed text-background/75">
                    {"This page has drifted out to sea. The address may have changed, or the page may no longer exist."}
                </p>
                <div class="mt-12 flex flex-wrap items-center gap-x-10 gap-y-4">
                    <a
                        href="/"
                        class="group inline-flex items-center gap-3 border border-background/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] text-background transition-colors duration-500 hover:border-background"
                    >
                        {"Return home"}
                        <span class="inline-block h-px w-10 bg-background transition-all duration-500 group-hover:w-16" />
                    </a>
                    <a
                        href="/buyers"
                        class="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.22em] text-background/80 transition-colors hover:text-background"
                    >
                        {"Explore properties"}
                        <span class="inline-block h-px w-8 bg-background/60 transition-all duration-500 group-hover:w-14" />
                    </a>
                </div>
            </div>
        </section>
    }
}
