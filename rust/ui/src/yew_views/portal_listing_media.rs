//! Listing Media — attach photos to a property.
//!
//! Clients-shaped workspace. The file blob never enters the Model; the effect
//! reads the input and posts multipart to /api/property-media/upload.
//!
//! Not registered in mod.rs until OPS-RECORDS-MEDIA-01 types land in model.rs.

use yew::prelude::*;

use crate::model::{Msg, PortalListingMediaPage, PortalListingProperty};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct ListingMediaProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct ListingMedia;

impl Component for ListingMedia {
    type Message = ();
    type Properties = ListingMediaProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("property-media").expect("listing media screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { workspace(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalListingMediaPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.listing_media.as_ref())
}

fn workspace(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let data = payload(model);
    let rows = data.map(|page| page.properties.as_slice()).unwrap_or(&[]);
    let total = data.map(|page| page.total).unwrap_or(0);
    let current = data.map(|page| page.page).unwrap_or(1);
    let page_size = data.map(|page| page.page_size.max(1)).unwrap_or(50);
    let pages = ((total + page_size - 1) / page_size).max(1);

    let oninput = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::QueryChanged(value));
        })
    };
    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(-1)))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::PageChanged(1)))
    };

    html! {
        <div class="grid min-h-0 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[220px_minmax(0,1fr)]">
            <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                    <div class="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("Listings · {total}") }
                    </div>
                    <input
                        type="search"
                        {oninput}
                        value={model.controls.query.clone()}
                        placeholder="Search…"
                        class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
                    />
                </div>
                <div class="min-h-0 flex-1 overflow-y-auto">
                    if rows.is_empty() {
                        <p class="px-3 py-6 text-sm font-light text-black/40">
                            { if model.loading { "Loading…" } else { "No matching listings." } }
                        </p>
                    } else {
                        { for rows.iter().map(|row| row_view(row, data.and_then(|d| d.selected_id.as_deref()), on_msg)) }
                    }
                </div>
                <div class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-1.5">
                    <button type="button" onclick={previous} disabled={current <= 1}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"← Prev"}
                    </button>
                    <span class="text-[10px] font-light text-black/40">{ format!("{current} / {pages}") }</span>
                    <button type="button" onclick={next} disabled={current >= pages}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"Next →"}
                    </button>
                </div>
            </aside>
            <div class="min-h-0 overflow-hidden">
                { selected_workspace(model, on_msg) }
            </div>
        </div>
    }
}

fn row_view(
    row: &PortalListingProperty,
    selected_id: Option<&str>,
    on_msg: &Callback<Msg>,
) -> Html {
    let selected = selected_id == Some(row.id.as_str());
    let id = row.id.clone();
    let onclick = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::RowSelected(id.clone())))
    };
    html! {
        <button type="button" {onclick}
            class={classes!(
                "flex","w-full","items-center","gap-2","border-b","border-[var(--portal-panel-border)]","px-2.5","py-2","text-left","transition",
                if selected {
                    "border-l-2 border-l-[var(--portal-gold)] bg-white/40"
                } else {
                    "border-l-2 border-l-transparent hover:bg-white/25"
                }
            )}>
            <div class="min-w-0 flex-1">
                <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">{ row.name.clone() }</div>
                <div class="truncate text-[11px] font-light text-black/45">
                    { format!("{} · {} photos", row.status, row.image_count) }
                </div>
            </div>
        </button>
    }
}

fn selected_workspace(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(data) = payload(model) else {
        return empty(model.loading);
    };
    let Some(property) = data.selected.as_ref() else {
        return empty(model.loading);
    };

    let role = model.listing_media.role.clone();
    let alt = model.listing_media.alt.clone();
    let on_role = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            on_msg.emit(Msg::ListingMediaRoleChanged(value));
        })
    };
    let on_alt = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::ListingMediaAltChanged(value));
        })
    };
    let on_file = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let input = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            let name = input
                .files()
                .and_then(|files| files.get(0))
                .map(|file| file.name())
                .unwrap_or_default();
            on_msg.emit(Msg::ListingMediaFileChosen(name));
        })
    };
    let on_upload = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::ListingMediaUploadRequested))
    };

    html! {
        <div class="flex h-full min-h-0 flex-col gap-3">
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{"Listing"}</div>
                <h2 class="mt-2 font-serif text-2xl font-light text-[var(--portal-navy)]">{ property.name.clone() }</h2>
                <p class="mt-1 text-xs font-light text-black/50">{ format!("{} · {} photos on file", property.status, property.image_count) }</p>
            </section>
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                <h3 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Add photo"}</h3>
                <div class="mt-4 grid gap-3 sm:grid-cols-2">
                    <label class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
                        {"Role"}
                        <select onchange={on_role} value={role.clone()}
                            class="mt-1 block w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-2.5 py-2 text-sm font-light">
                            <option value="gallery">{"Gallery"}</option>
                            <option value="hero">{"Hero"}</option>
                        </select>
                    </label>
                    <label class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
                        {"Alt text"}
                        <input type="text" value={alt} oninput={on_alt}
                            class="mt-1 block w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-2.5 py-2 text-sm font-light" />
                    </label>
                </div>
                <input id="listing-media-file" type="file" accept="image/*" onchange={on_file}
                    class="mt-4 block w-full text-sm font-light" />
                <div class="mt-4 flex items-center gap-3">
                    <button type="button" onclick={on_upload} disabled={model.loading}
                        class="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-45">
                        {"Upload"}
                    </button>
                    <span class="text-xs font-light text-black/45">
                        { model.listing_media.file_name.clone().unwrap_or_else(|| "No file chosen".into()) }
                    </span>
                </div>
            </section>
        </div>
    }
}

fn empty(loading: bool) -> Html {
    html! {
        <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
            <p class="text-sm font-light text-black/45">
                { if loading { "Loading listing…" } else { "Select a listing." } }
            </p>
        </section>
    }
}
