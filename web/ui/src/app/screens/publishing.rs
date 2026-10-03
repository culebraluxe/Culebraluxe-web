//! MARKETING redesigned as an operational Publishing Center.
//! Read-only by design: property/media remain the writers. This screen shows readiness and routes edits to them.

use yew::prelude::*;

use crate::app::api::PublishingRead;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalPage, PortalPublishingListing, PortalPublishingPage};

pub struct Publishing;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalPublishingPage>,
    pub selected_id: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    Selected(String),
}

impl Screen for Publishing {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::request(PublishingRead, Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => match answer.and_then(|page| {
                page.publishing
                    .ok_or_else(|| ApiError::decode("The answer had no Publishing payload in it."))
            }) {
                Ok(page) => {
                    if model.selected_id.as_ref().is_none_or(|id| {
                        !page
                            .listings
                            .iter()
                            .any(|listing| &listing.property_id == id)
                    }) {
                        model.selected_id = page
                            .listings
                            .first()
                            .map(|listing| listing.property_id.clone());
                    }
                    model.read = Remote::Loaded(page);
                }
                Err(error) => model.read = Remote::Failed(error),
            },
            Msg::Selected(id) => model.selected_id = Some(id),
        }
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        html! {
            <div class="space-y-6">
                { template::portal_heading(
                    "Marketing",
                    "Publishing",
                    "One operational view of listing copy, media, CulebraLuxe.com, Facebook readiness and the Stellar submission package.",
                ) }
                { template::remote(&model.read, "Publishing", |page| workspace(page, model, &on_msg)) }
            </div>
        }
    }
}

fn workspace(page: &PortalPublishingPage, model: &Model, on_msg: &Callback<Msg>) -> Html {
    let selected = model
        .selected_id
        .as_deref()
        .and_then(|id| {
            page.listings
                .iter()
                .find(|listing| listing.property_id == id)
        })
        .or_else(|| page.listings.first());

    html! {
        <>
            <div class="grid gap-4 sm:grid-cols-3">
                { template::metric("Listings", &page.listings.len().to_string(), "Active or publication-relevant property records.") }
                { template::metric("Package ready", &page.ready_count.to_string(), "Website and Stellar readiness checks complete.") }
                { template::metric("Live", &page.live_count.to_string(), "Currently published on CulebraLuxe.com.") }
            </div>

            if page.listings.is_empty() {
                { template::empty_panel("No active listing is ready for the publishing workflow.") }
            } else {
                <section class="grid min-h-[560px] overflow-hidden rounded-[var(--portal-panel-radius)] portal-glass-panel lg:grid-cols-[minmax(300px,0.85fr)_minmax(0,1.4fr)]">
                    <div class="border-b border-[var(--portal-border)] lg:border-b-0 lg:border-r">
                        { for page.listings.iter().map(|listing| listing_row(listing, selected.map(|item| item.property_id.as_str()), on_msg)) }
                    </div>
                    <div class="p-6">
                        { selected.map(detail).unwrap_or_default() }
                    </div>
                </section>
            }
        </>
    }
}

fn listing_row(
    listing: &PortalPublishingListing,
    selected: Option<&str>,
    on_msg: &Callback<Msg>,
) -> Html {
    let id = listing.property_id.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::Selected(id.clone())))
    };
    let active = selected == Some(listing.property_id.as_str());
    html! {
        <button type="button" {onclick}
            class={classes!(
                "block", "w-full", "border-b", "border-[var(--portal-border)]", "px-5", "py-4", "text-left",
                if active { "bg-white/45" } else { "hover:bg-white/25" }
            )}>
            <div class="flex items-center justify-between gap-3">
                <span class="truncate font-serif text-lg font-light text-[var(--portal-navy)]">{ listing.name.clone() }</span>
                <span class={classes!(
                    "h-2", "w-2", "shrink-0", "rounded-full",
                    if listing.is_published { "bg-emerald-500" } else if listing.website_ready { "bg-amber-400" } else { "bg-black/20" }
                )}></span>
            </div>
            <p class="mt-1 truncate text-xs font-light text-black/45">{ listing.location.clone().unwrap_or_else(|| "Location not recorded".into()) }</p>
            <p class="mt-2 text-[9px] font-light uppercase tracking-[0.12em] text-black/35">
                { if listing.is_published { "Live" } else if listing.website_ready { "Ready" } else { "Preparing" } }
                {" · "}
                { format!("{} photos · {} video", listing.image_count, listing.video_count) }
            </p>
        </button>
    }
}

fn detail(listing: &PortalPublishingListing) -> Html {
    html! {
        <div class="space-y-6">
            <header>
                <p class="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{"Publishing workspace"}</p>
                <h2 class="mt-1 font-serif text-3xl font-light">{ listing.name.clone() }</h2>
                <p class="mt-1 text-sm font-light text-black/45">
                    { listing.location.clone().unwrap_or_else(|| "Location not recorded".into()) }
                </p>
            </header>

            <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
                { readiness("Listing copy", listing.copy_ready, "Description + SEO") }
                { readiness("Media", listing.media_ready, if listing.has_hero { "Hero + photos" } else { "Hero missing" }) }
                { readiness("CulebraLuxe.com", listing.website_ready, if listing.is_published { "Live" } else { "Ready state" }) }
                { readiness("Facebook", listing.facebook_ready, "Marketplace package") }
                { readiness("Stellar MLS", listing.stellar_package_ready, "Submission package") }
                { readiness("Active listing", listing.is_active_listing, &title_case(&listing.status)) }
            </div>

            if !listing.missing.is_empty() {
                <section class="rounded-[var(--portal-panel-radius)] bg-white/35 p-5">
                    <p class="text-[10px] font-light uppercase tracking-[0.16em] text-black/40">{"Missing before full publication"}</p>
                    <div class="mt-3 flex flex-wrap gap-2">
                        { for listing.missing.iter().map(|item| html! {
                            <span class="rounded-full border border-[var(--portal-border)] px-2.5 py-1 text-[10px] font-light text-black/55">{ item.clone() }</span>
                        }) }
                    </div>
                </section>
            }

            <section class="grid gap-3 sm:grid-cols-2">
                <a href={format!("/portal/property-admin/{}", listing.property_id)}
                    class="flex min-h-14 items-center justify-between rounded-[var(--portal-panel-radius)] border border-[var(--portal-border)] bg-white/35 px-4 text-sm font-light text-[var(--portal-navy)] hover:bg-white/60">
                    <span>{"Edit property facts & copy"}</span><span>{"→"}</span>
                </a>
                <a href="/portal/property-media"
                    class="flex min-h-14 items-center justify-between rounded-[var(--portal-panel-radius)] border border-[var(--portal-border)] bg-white/35 px-4 text-sm font-light text-[var(--portal-navy)] hover:bg-white/60">
                    <span>{"Manage listing media"}</span><span>{"→"}</span>
                </a>
                if let Some(slug) = listing.slug.as_ref().filter(|_| listing.is_published) {
                    <a href={format!("/properties/{slug}")} target="_blank"
                        class="flex min-h-14 items-center justify-between rounded-[var(--portal-panel-radius)] border border-[var(--portal-border)] bg-white/35 px-4 text-sm font-light text-[var(--portal-navy)] hover:bg-white/60">
                        <span>{"Open live property"}</span><span>{"↗"}</span>
                    </a>
                }
                <div class="rounded-[var(--portal-panel-radius)] border border-[var(--portal-border)] bg-white/20 px-4 py-3">
                    <p class="text-[9px] font-light uppercase tracking-[0.14em] text-black/35">{"Stellar"}</p>
                    <p class="mt-1 text-sm font-light text-black/60">
                        { listing.listing_type.clone().unwrap_or_else(|| "Listing type missing".into()) }
                        {" · "}
                        { listing.agent_mls_id.clone().unwrap_or_else(|| "MLS ID missing".into()) }
                    </p>
                </div>
            </section>
        </div>
    }
}

fn readiness(label: &str, ready: bool, detail: &str) -> Html {
    html! {
        <div class="rounded-[var(--portal-panel-radius)] border border-[var(--portal-border)] bg-white/30 p-4">
            <div class="flex items-center justify-between gap-3">
                <p class="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">{ label.to_owned() }</p>
                <span class={classes!(
                    "rounded-full", "px-2", "py-0.5", "text-[9px]", "font-medium", "uppercase", "tracking-[0.1em]",
                    if ready { "bg-emerald-100 text-emerald-800" } else { "bg-amber-100 text-amber-800" }
                )}>{ if ready { "Ready" } else { "Needs work" } }</span>
            </div>
            <p class="mt-2 text-xs font-light text-black/50">{ detail.to_owned() }</p>
        </div>
    }
}

fn title_case(value: &str) -> String {
    value
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishing_reads_its_one_projection() {
        let (_model, cmd) = Publishing::init(&ScreenCtx::default());
        assert_eq!(
            cmd.into_requests().remove(0).path,
            "/api/portal/rust-ui/publishing"
        );
    }
}
