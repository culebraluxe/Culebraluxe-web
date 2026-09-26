//! The site building blocks — the public-page read, `SitePage<T>` for a page that shows it, `StaticPage<T>` for a page
//! whose words are its own, and the pieces every site page shares (the interior hero, the interruption).
//!
//! A SITE PAGE LOADS QUIETLY. The portal shows "Reading…" panels; a visitor sees the page arrive. Loading is an empty
//! stretch of the page's height, and a failure is the site's own interruption, never a portal error box.

use std::marker::PhantomData;

use yew::prelude::*;

use crate::app::api::PublicPage;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::model::PageContent;

/// A page that shows the public page payload.
pub trait SitePageSpec: 'static {
    /// The registry key the page read names.
    const SCREEN: &'static str;
    /// Whether the read is about the record in the URL (a property's slug).
    const SCOPED: bool = false;
    fn view(page: &PageContent, ctx: &ScreenCtx) -> Html;
}

pub struct SitePage<T: SitePageSpec>(PhantomData<T>);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PageContent>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PageContent, ApiError>),
}

pub fn read(screen: &'static str, scope: Option<String>) -> PublicPage {
    PublicPage { screen, scope }
}

impl<T: SitePageSpec> Screen for SitePage<T> {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let scope = if T::SCOPED { ctx.id.clone() } else { None };
        (
            Model {
                read: Remote::Loading,
            },
            Cmd::request(read(T::SCREEN, scope), Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        let Msg::Loaded(answer) = msg;
        model.read = Remote::from_result(answer);
        Cmd::none()
    }

    fn view(model: &Model, ctx: &ScreenCtx, _link: &Link<Msg>) -> Html {
        remote(&model.read, |page| T::view(page, ctx))
    }
}

/// A site read in its three states, the site's way.
pub fn remote<T>(read: &Remote<T>, loaded: impl FnOnce(&T) -> Html) -> Html {
    match read {
        Remote::Loaded(value) => loaded(value),
        Remote::Failed(_) => html! { <ErrorView /> },
        Remote::Loading | Remote::NotAsked => {
            html! { <div class="min-h-[80svh]" data-screen-state="loading" aria-busy="true"></div> }
        }
    }
}

/// A page whose words are written into it: nothing to read.
pub trait StaticPageSpec: 'static {
    fn view(ctx: &ScreenCtx) -> Html;
}

pub struct StaticPage<T: StaticPageSpec>(PhantomData<T>);

impl<T: StaticPageSpec> Screen for StaticPage<T> {
    type Model = ();
    type Msg = ();

    fn init(_ctx: &ScreenCtx) -> ((), Cmd<()>) {
        ((), Cmd::none())
    }

    fn update(_model: &mut (), _msg: (), _ctx: &ScreenCtx) -> Cmd<()> {
        Cmd::none()
    }

    fn view(_model: &(), ctx: &ScreenCtx, _link: &Link<()>) -> Html {
        T::view(ctx)
    }
}

/// `components/page-hero.tsx` — the header every interior page opens with.
///
/// One component, five pages, so it is written once here too: the height, the padding that clears the fixed header, the
/// scrim that makes ivory text legible over a photograph, and the rule that an absent intro renders nothing.
pub fn page_hero(
    eyebrow: &str,
    title: &str,
    intro: Option<&str>,
    image: &str,
    image_alt: &str,
) -> Html {
    let intro = intro.map(|intro| {
        html! {
            <p class="mt-8 max-w-2xl text-pretty text-base font-light leading-relaxed text-background/80 md:text-lg">
                { intro }
            </p>
        }
    });
    html! {
        <section class="relative flex min-h-[68svh] items-end overflow-hidden">
            <img src={image.to_string()} alt={image_alt.to_string()} sizes="100vw"
                class="absolute inset-0 h-full w-full object-cover" />
            <div class="absolute inset-0 bg-gradient-to-t from-black/70 via-black/25 to-black/40"></div>
            <div class="relative w-full px-6 pb-16 pt-40 md:px-12 md:pb-24">
                <div class="mx-auto max-w-[1600px]">
                    // The live hero faded up on load (`Reveal` fires at once for what is already in view), heading
                    // first and the intro a beat after.
                    <div class="animate-[fadeUp_1.2s_cubic-bezier(0.22,1,0.36,1)_both]">
                        <p class="mb-5 text-xs font-light uppercase tracking-[0.4em] text-background/70">{ eyebrow }</p>
                        <h1 class="max-w-4xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
                            { title }
                        </h1>
                    </div>
                    <div class="animate-[fadeUp_1.2s_cubic-bezier(0.22,1,0.36,1)_both] [animation-delay:120ms]">{ intro }</div>
                </div>
            </div>
        </section>
    }
}

/// The interruption, in the design's own words.
///
/// THE RETRY IS A LINK BACK TO THIS SAME URL, and that is a deliberate trade rather than an oversight. Next's error
/// boundary hands a client component a `reset()` that re-renders the segment in place; reaching it from here would mean
/// a callback crossing from React into this view, which is the kind of seam this port removes. An empty `href` resolves
/// to the current document, so the link reloads the page and retries — heavier than `reset`, always correct, and it
/// needs no JavaScript at all.
#[function_component(ErrorView)]
pub fn error_view() -> Html {
    html! {
        <section class="flex min-h-[80svh] items-center bg-foreground px-6 text-background md:px-12">
            <div class="mx-auto w-full max-w-[1600px]">
                <p class="mb-6 text-xs font-light uppercase tracking-[0.4em] text-background/60">{"CulebraLuxe"}</p>
                <h1 class="max-w-2xl text-balance font-serif text-4xl font-light leading-[1.05] text-background md:text-6xl">
                    {"Something drifted off course."}
                </h1>
                <p class="mt-6 max-w-xl text-pretty text-sm font-light leading-relaxed text-background/75">
                    {"A momentary interruption while we prepared this page. You can try again, or return to the CulebraLuxe home."}
                </p>
                <div class="mt-12 flex flex-wrap items-center gap-x-10 gap-y-4">
                    <a
                        href=""
                        class="group inline-flex min-h-11 items-center gap-3 border border-background/40 px-8 py-4 text-xs font-light uppercase tracking-[0.22em] text-background transition-colors duration-500 hover:border-background"
                    >
                        {"Try again"}
                        <span class="inline-block h-px w-10 bg-background transition-all duration-500 group-hover:w-16" />
                    </a>
                    <a
                        href="/"
                        class="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.22em] text-background/80 transition-colors hover:text-background"
                    >
                        {"Return home"}
                        <span class="inline-block h-px w-8 bg-background/60 transition-all duration-500 group-hover:w-14" />
                    </a>
                </div>
            </div>
        </section>
    }
}

/// One field of the form a submit event came from, by name. The form's fields are uncontrolled (as they were in the
/// live forms): what was typed is read once, when it is sent.
pub fn form_field(event: &SubmitEvent, name: &str) -> String {
    event
        .target_dyn_into::<web_sys::HtmlFormElement>()
        .and_then(|form| web_sys::FormData::new_with_form(&form).ok())
        .and_then(|data| data.get(name).as_string())
        .unwrap_or_default()
}
