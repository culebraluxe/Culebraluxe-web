//! OPPS — Listing Media (`/portal/property-media`): pick a listing, add a photograph to it.
//!
//! The list (search with a pause, paging, selection) and the upload are the screen's. The photograph is sent with the
//! chunked-upload command, like the Workbench's, so a large file is not refused by the gateway.

use yew::prelude::*;

use crate::app::api::{ListingMediaRead, PROPERTY_MEDIA_CHUNKED};
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalListingMediaPage, PortalListingProperty, PortalPage};

const SEARCH_PAUSE_MS: u32 = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub read: Remote<PortalListingMediaPage>,
    pub loading: bool,
    pub selected: Option<String>,
    pub query: String,
    /// 0-based.
    pub page: usize,
    pub role: String,
    pub alt: String,
    pub file_name: Option<String>,
    pub uploading: bool,
    pub notice: Option<crate::model::CommandNotice>,
    seq: u32,
    typed: u32,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            read: Remote::NotAsked,
            loading: false,
            selected: None,
            query: String::new(),
            page: 0,
            role: "gallery".into(),
            alt: String::new(),
            file_name: None,
            uploading: false,
            notice: None,
            seq: 0,
            typed: 0,
        }
    }
}

#[derive(Debug)]
pub enum Msg {
    Loaded {
        seq: u32,
        answer: Result<PortalPage, ApiError>,
    },
    QueryChanged(String),
    SearchPaused(u32),
    PageChanged(i64),
    RowSelected(String),
    RoleChanged(String),
    AltChanged(String),
    FileChosen(Option<web_sys::File>),
    Uploaded(Result<(), ApiError>),
}

pub struct ListingMedia;

fn read(model: &mut Model) -> Cmd<Msg> {
    model.seq += 1;
    model.loading = true;
    let seq = model.seq;
    Cmd::request(
        ListingMediaRead {
            selected: model.selected.clone(),
            search: model.query.clone(),
            page: model.page,
        },
        move |answer| Msg::Loaded { seq, answer },
    )
}

impl Screen for ListingMedia {
    type Model = Model;
    type Msg = Msg;

    fn init(ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        let mut model = Model {
            read: Remote::Loading,
            selected: ctx.query("selected").map(str::to_owned),
            ..Model::default()
        };
        let read = read(&mut model);
        (model, read)
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded { seq, answer } => {
                if seq != model.seq {
                    return Cmd::none();
                }
                model.loading = false;
                match answer.and_then(|page| {
                    page.listing_media
                        .ok_or_else(|| ApiError::decode("The answer had no listings in it."))
                }) {
                    Ok(page) => {
                        model.selected = page.selected_id.clone();
                        model.read = Remote::Loaded(page);
                    }
                    Err(error) if model.read.loaded().is_some() => {
                        model.notice = Some(crate::model::CommandNotice::failure(error.message))
                    }
                    Err(error) => model.read = Remote::Failed(error),
                }
                Cmd::none()
            }
            Msg::QueryChanged(query) => {
                model.query = query;
                model.typed += 1;
                Cmd::after(SEARCH_PAUSE_MS, Msg::SearchPaused(model.typed))
            }
            Msg::SearchPaused(typed) if typed == model.typed => {
                model.page = 0;
                model.selected = None;
                read(model)
            }
            Msg::SearchPaused(_) => Cmd::none(),
            Msg::PageChanged(delta) => {
                let pages = model
                    .read
                    .loaded()
                    .map(|page| {
                        let size = page.page_size.max(1);
                        ((page.total + size - 1) / size).max(1)
                    })
                    .unwrap_or(1);
                let next = (model.page as i64)
                    .saturating_add(delta)
                    .clamp(0, pages - 1) as usize;
                if next == model.page {
                    return Cmd::none();
                }
                model.page = next;
                model.selected = None;
                read(model)
            }
            Msg::RowSelected(id) => {
                if !model
                    .read
                    .loaded()
                    .is_some_and(|page| page.properties.iter().any(|row| row.id == id))
                {
                    return Cmd::none();
                }
                model.selected = Some(id);
                model.notice = None;
                read(model)
            }
            Msg::RoleChanged(role) => {
                if matches!(role.as_str(), "hero" | "gallery") {
                    model.role = role;
                }
                Cmd::none()
            }
            Msg::AltChanged(alt) => {
                model.alt = alt;
                Cmd::none()
            }
            Msg::FileChosen(file) => {
                let Some(file) = file else {
                    return Cmd::none();
                };
                if model.uploading {
                    return Cmd::none();
                }
                let Some(property_id) = model.selected.clone() else {
                    model.notice = Some(crate::model::CommandNotice::failure(
                        "Select a listing before uploading.",
                    ));
                    return Cmd::none();
                };
                model.file_name = Some(file.name());
                model.uploading = true;
                model.notice = None;
                let mut init = vec![("role".to_string(), model.role.clone())];
                if !model.alt.trim().is_empty() {
                    init.push(("altText".to_string(), model.alt.trim().to_string()));
                }
                Cmd::upload(
                    file,
                    PROPERTY_MEDIA_CHUNKED,
                    vec![("propertyId".to_string(), property_id)],
                    init,
                    Msg::Uploaded,
                )
            }
            Msg::Uploaded(result) => {
                model.uploading = false;
                match result {
                    Ok(()) => {
                        model.notice = Some(crate::model::CommandNotice::success(format!(
                            "{} was added.",
                            model.file_name.take().unwrap_or_else(|| "The photo".into())
                        )));
                        model.alt.clear();
                        read(model)
                    }
                    Err(error) => {
                        model.notice = Some(crate::model::CommandNotice::failure(format!(
                            "The photo was not added: {}",
                            error.message
                        )));
                        Cmd::none()
                    }
                }
            }
        }
    }

    fn view(model: &Model, ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        template::remote(&model.read, "the listings", |page| {
            workspace(model, page, ctx, link)
        })
    }
}

fn workspace(
    model: &Model,
    page: &PortalListingMediaPage,
    ctx: &ScreenCtx,
    link: &Link<Msg>,
) -> Html {
    let size = page.page_size.max(1);
    let pages = ((page.total + size - 1) / size).max(1);
    let current = model.page as i64 + 1;
    let oninput =
        link.callback(|event: InputEvent| Msg::QueryChanged(template::input_value(&event)));
    html! {
        <div class="grid min-h-0 gap-4 md:h-[calc(100dvh-8.5rem)] md:grid-cols-[220px_minmax(0,1fr)]">
            <aside class="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
                <div class="shrink-0 border-b border-[var(--portal-panel-border)] p-2.5">
                    <div class="mb-2 text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                        { format!("Listings · {}", page.total) }
                    </div>
                    <input type="search" {oninput} value={model.query.clone()} placeholder="Search…"
                        class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]" />
                </div>
                <div class="min-h-0 flex-1 overflow-y-auto">
                    if page.properties.is_empty() {
                        <p class="px-3 py-6 text-sm font-light text-black/40">
                            { if model.loading { "Loading…" } else { "No matching listings." } }
                        </p>
                    } else {
                        { for page.properties.iter().map(|row| row_view(row, page.selected_id.as_deref(), link)) }
                    }
                </div>
                <div class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-2 py-1.5">
                    <button type="button" onclick={link.callback(|_: MouseEvent| Msg::PageChanged(-1))} disabled={current <= 1}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"← Prev"}
                    </button>
                    <span class="text-[10px] font-light text-black/40">{ format!("{current} / {pages}") }</span>
                    <button type="button" onclick={link.callback(|_: MouseEvent| Msg::PageChanged(1))} disabled={current >= pages}
                        class="text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] disabled:opacity-30">
                        {"Next →"}
                    </button>
                </div>
            </aside>
            <div class="min-h-0 overflow-hidden">
                { selected(model, page, ctx, link) }
            </div>
        </div>
    }
}

fn row_view(row: &PortalListingProperty, selected_id: Option<&str>, link: &Link<Msg>) -> Html {
    let selected = selected_id == Some(row.id.as_str());
    let id = row.id.clone();
    html! {
        <button type="button" onclick={link.callback(move |_: MouseEvent| Msg::RowSelected(id.clone()))}
            class={classes!(
                "flex","w-full","items-center","gap-2","border-b","border-[var(--portal-panel-border)]","px-2.5","py-2","text-left","transition",
                if selected { "border-l-2 border-l-[var(--portal-gold)] bg-white/40" } else { "border-l-2 border-l-transparent hover:bg-white/25" }
            )}>
            <div class="min-w-0 flex-1">
                <div class="truncate text-[13px] font-medium text-[var(--portal-navy)]">{ row.name.clone() }</div>
                <div class="truncate text-[11px] font-light text-black/45">{ format!("{} · {} photos", row.status, row.image_count) }</div>
            </div>
        </button>
    }
}

fn selected(
    model: &Model,
    page: &PortalListingMediaPage,
    ctx: &ScreenCtx,
    link: &Link<Msg>,
) -> Html {
    let Some(property) = page.selected.as_ref() else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
                <p class="text-sm font-light text-black/45">{"Select a listing."}</p>
            </section>
        };
    };
    let on_role = link.callback(|event: Event| {
        Msg::RoleChanged(
            event
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value(),
        )
    });
    let on_alt = link.callback(|event: InputEvent| Msg::AltChanged(template::input_value(&event)));
    let on_file = link.callback(|event: Event| {
        let input = event.target_unchecked_into::<web_sys::HtmlInputElement>();
        let file = input.files().and_then(|files| files.get(0));
        input.set_value("");
        Msg::FileChosen(file)
    });
    html! {
        <div class="flex h-full min-h-0 flex-col gap-3">
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                <div class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">{"Listing"}</div>
                <h2 class="mt-2 font-serif text-2xl font-light text-[var(--portal-navy)]">{ property.name.clone() }</h2>
                <p class="mt-1 text-xs font-light text-black/50">{ format!("{} · {} photos on file", property.status, property.image_count) }</p>
            </section>
            if ctx.can("property.write") {
                <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-4">
                    <h3 class="font-serif text-xl font-light text-[var(--portal-navy)]">{"Add photo"}</h3>
                    <div class="mt-4 grid gap-3 sm:grid-cols-2">
                        <label class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
                            {"Role"}
                            <select onchange={on_role}
                                class="mt-1 block w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-2.5 py-2 text-sm font-light">
                                <option value="gallery" selected={model.role == "gallery"}>{"Gallery"}</option>
                                <option value="hero" selected={model.role == "hero"}>{"Hero"}</option>
                            </select>
                        </label>
                        <label class="text-[10px] font-light uppercase tracking-[0.16em] text-black/35">
                            {"Alt text"}
                            <input type="text" value={model.alt.clone()} oninput={on_alt}
                                class="mt-1 block w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/55 px-2.5 py-2 text-sm font-light" />
                        </label>
                    </div>
                    // Choosing the file IS the upload: role and alt text are set first.
                    <label class={classes!("mt-4", "inline-flex", "h-10", "cursor-pointer", "items-center", "rounded-[var(--portal-tab-radius)]",
                        "bg-[var(--portal-navy)]", "px-4", "text-[10px]", "font-medium", "uppercase", "tracking-[0.14em]", "text-white",
                        model.uploading.then_some("pointer-events-none opacity-45"))}>
                        { if model.uploading { "Uploading…" } else { "Choose photo & upload" } }
                        <input type="file" accept="image/*" onchange={on_file} class="hidden" disabled={model.uploading} />
                    </label>
                    if let Some(notice) = &model.notice {
                        <p class={classes!("mt-3", "text-xs", "font-light", if notice.ok { "text-emerald-700" } else { "text-red-700" })} role="status">
                            { notice.message.clone() }
                        </p>
                    }
                </section>
            }
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn it_searches_after_a_pause_and_only_the_newest_answer_counts() {
        let ctx = ScreenCtx::default();
        let (mut model, cmd) = ListingMedia::init(&ctx);
        let first = cmd.into_requests().remove(0);
        assert_eq!(
            first.path,
            "/api/portal/rust-ui/listing-media?page=0&search="
        );
        ListingMedia::update(&mut model, Msg::QueryChanged("vil".into()), &ctx);
        ListingMedia::update(&mut model, Msg::QueryChanged("villa".into()), &ctx);
        assert!(
            ListingMedia::update(&mut model, Msg::SearchPaused(1), &ctx)
                .into_requests()
                .is_empty(),
            "a superseded pause does nothing"
        );
        let search = ListingMedia::update(&mut model, Msg::SearchPaused(2), &ctx)
            .into_requests()
            .remove(0);
        assert_eq!(
            search.path,
            "/api/portal/rust-ui/listing-media?page=0&search=villa"
        );
        // The first read answers late: it is dropped.
        ListingMedia::update(
            &mut model,
            first.respond(Ok(
                json!({ "listingMedia": { "properties": [{ "id": "old" }] } }),
            )),
            &ctx,
        );
        assert!(model.read.loaded().is_none());
        ListingMedia::update(
            &mut model,
            search.respond(Ok(
                json!({ "listingMedia": { "properties": [{ "id": "p1" }], "total": 1 } }),
            )),
            &ctx,
        );
        assert_eq!(model.read.loaded().unwrap().properties[0].id, "p1");
        assert!(
            ListingMedia::update(&mut model, Msg::FileChosen(None), &ctx)
                .into_requests()
                .is_empty()
        );
    }
}
