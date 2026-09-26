//! CORE — the Cabinet (`/portal/documents`): the repository of issued documents, filtered in place by deal, client,
//! property, type or issuer. A read; issuing happens in Forms.

use yew::prelude::*;

use crate::app::api::CabinetRead;
use crate::app::cmd::{ApiError, Cmd, Remote};
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::app::template;
use crate::model::{PortalCabinetDocument, PortalCabinetPage, PortalPage};

pub struct Cabinet;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub read: Remote<PortalCabinetPage>,
    /// The filter as typed. The whole repository is on the page, so filtering is a view of it, not a new read.
    pub query: String,
}

#[derive(Debug, PartialEq)]
pub enum Msg {
    Loaded(Result<PortalPage, ApiError>),
    QueryChanged(String),
}

impl Screen for Cabinet {
    type Model = Model;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (Model, Cmd<Msg>) {
        (
            Model {
                read: Remote::Loading,
                ..Model::default()
            },
            Cmd::request(CabinetRead, Msg::Loaded),
        )
    }

    fn update(model: &mut Model, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::Loaded(answer) => {
                model.read = Remote::from_result(answer.and_then(|page| {
                    page.cabinet
                        .ok_or_else(|| ApiError::decode("The answer had no Cabinet in it."))
                }))
            }
            Msg::QueryChanged(query) => model.query = query,
        }
        Cmd::none()
    }

    fn view(model: &Model, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        let on_msg = link.callback(|msg: Msg| msg);
        template::remote(&model.read, "the Cabinet", |data| {
            cabinet(data, &model.query, &on_msg)
        })
    }
}

fn visible_documents<'a>(
    documents: &'a [PortalCabinetDocument],
    query: &str,
) -> Vec<&'a PortalCabinetDocument> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return documents.iter().collect();
    }

    documents
        .iter()
        .filter(|document| {
            [
                document.deal_name.as_deref(),
                document.party_name.as_deref(),
                document.property_name.as_deref(),
                document.document_type_label.as_deref(),
                document.issued_by_display_name.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|value| value.to_ascii_lowercase().contains(&needle))
        })
        .collect()
}

fn cabinet(data: &PortalCabinetPage, query: &str, on_msg: &Callback<Msg>) -> Html {
    let visible = visible_documents(&data.documents, query);
    let oninput = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::QueryChanged(value));
        })
    };

    html! {
        <div>
            <header class="mb-8">
                <p class="text-xs font-light uppercase tracking-[0.28em] text-black/40">
                    {"NEXUS"}
                </p>
                <div class="mt-3 flex items-baseline gap-4">
                    <h1 class="font-serif text-4xl font-light leading-[1.1]">
                        {"Documents"}
                    </h1>
                    <span class="rounded-full bg-[var(--portal-blue-gray)]/15 px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-navy-soft)]">
                        {"Issued repository"}
                    </span>
                </div>
                <p class="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
                    {"Canonical issued artifacts only. Retrieval is by deal, client, property or document type — never folders. Issued documents are immutable; a revised issuance is a new version, not an overwrite."}
                </p>
            </header>

            <section class="portal-glass-panel overflow-hidden rounded-[var(--portal-panel-radius)]">
                <header class="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--portal-panel-border)] px-[var(--portal-panel-padding)] py-5">
                    <div class="min-w-0">
                        <h2 class="font-serif text-2xl font-light text-[var(--portal-panel-heading)]">
                            {"Repository"}
                        </h2>
                        <p class="mt-1 text-sm font-light text-black/45">
                            { format!(
                                "{} issued document{}",
                                data.documents.len(),
                                if data.documents.len() == 1 { "" } else { "s" }
                            ) }
                        </p>
                    </div>

                    <div class="w-full max-w-xs space-y-2">
                        <label class="block text-[10px] font-light uppercase tracking-[0.18em] text-black/45">
                            {"Filter by deal / client / property"}
                        </label>
                        <input
                            type="search"
                            {oninput}
                            value={query.to_string()}
                            placeholder="Search…"
                            class="min-h-11 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 text-sm font-light text-[var(--portal-text)] outline-none transition placeholder:text-black/35 hover:border-[var(--portal-blue-gray)]/60 focus:border-[var(--portal-navy-soft)] focus:ring-1 focus:ring-[var(--portal-gold)]/35"
                        />
                    </div>
                </header>

                <div class="hidden md:block">
                    <div class="overflow-x-auto [scrollbar-width:thin]">
                        <table class="w-full min-w-[760px] border-collapse text-left text-sm">
                            <thead class="bg-[var(--portal-blue-pale)]">
                                <tr class="border-b border-[var(--portal-border)]">
                                    { for [
                                        "Document",
                                        "Deal / Client",
                                        "Version",
                                        "Issued",
                                        "By",
                                        "Status",
                                        "PDF",
                                    ].into_iter().map(table_header) }
                                </tr>
                            </thead>
                            <tbody>
                                if visible.is_empty() {
                                    <tr class="border-b border-[var(--portal-border)] last:border-b-0">
                                        <td colspan="7" class="px-4 py-6 font-light italic text-black/40 sm:px-6">
                                            {"No issued documents yet — assemble a form in NEXUS · Forms and issue it."}
                                        </td>
                                    </tr>
                                } else {
                                    { for visible.iter().map(|document| desktop_row(document)) }
                                }
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="divide-y divide-[var(--portal-border)] md:hidden">
                    if visible.is_empty() {
                        <p class="px-4 py-6 text-sm font-light italic text-black/40">
                            {"No issued documents yet — assemble a form in NEXUS · Forms and issue it."}
                        </p>
                    } else {
                        { for visible.iter().map(|document| mobile_card(document)) }
                    }
                </div>
            </section>
        </div>
    }
}

fn table_header(label: &'static str) -> Html {
    html! {
        <th class="px-4 py-3 text-[10px] font-light uppercase tracking-[0.18em] text-black/45 sm:px-6">
            { label }
        </th>
    }
}

fn desktop_row(document: &&PortalCabinetDocument) -> Html {
    html! {
        <tr class="border-b border-[var(--portal-border)] transition-colors last:border-b-0 hover:bg-[var(--portal-blue-pale)]/50">
            <td class="px-4 py-3.5 font-light text-black/60 sm:px-6">
                <div class="font-light text-[var(--portal-navy)]">
                    { document_label(document) }
                </div>
                if let Some(title) = document.title.as_deref() {
                    <div class="mt-0.5 text-xs font-light text-black/45">{ title }</div>
                }
            </td>
            <td class="px-4 py-3.5 font-light leading-5 text-black/60 sm:px-6">
                { party_or_deal(document) }
                if let Some(property_name) = document.property_name.as_deref() {
                    <div class="text-xs text-black/40">{ property_name }</div>
                }
            </td>
            <td class="px-4 py-3.5 font-light text-black/60 sm:px-6">
                { version_label(document.issued_version) }
            </td>
            <td class="px-4 py-3.5 text-xs font-light text-black/45 sm:px-6">
                { date_label(&document.created_at) }
            </td>
            <td class="px-4 py-3.5 text-xs font-light text-black/45 sm:px-6">
                { document.issued_by_display_name.as_deref().unwrap_or("—") }
            </td>
            <td class="px-4 py-3.5 font-light text-black/60 sm:px-6">
                { state_pill(&document.state) }
            </td>
            <td class="px-4 py-3.5 font-light text-black/60 sm:px-6">
                { download_links(document, false) }
            </td>
        </tr>
    }
}

fn mobile_card(document: &&PortalCabinetDocument) -> Html {
    html! {
        <article class="space-y-3 px-4 py-4">
            <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                    <h3 class="font-light text-[var(--portal-navy)]">
                        { document_label(document) }
                    </h3>
                    if let Some(title) = document.title.as_deref() {
                        <p class="mt-0.5 text-xs font-light text-black/45">{ title }</p>
                    }
                </div>
                { state_pill(&document.state) }
            </div>

            <dl class="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                <div class="col-span-2">
                    <dt class="font-light uppercase tracking-[0.12em] text-black/35">
                        {"Deal / Client"}
                    </dt>
                    <dd class="mt-1 font-light text-black/65">
                        { party_or_deal(document) }
                        if let Some(property_name) = document.property_name.as_deref() {
                            <span class="text-black/40">{ format!(" · {property_name}") }</span>
                        }
                    </dd>
                </div>
                <div>
                    <dt class="font-light uppercase tracking-[0.12em] text-black/35">{"Version"}</dt>
                    <dd class="mt-1 font-light text-black/65">
                        { version_label(document.issued_version) }
                    </dd>
                </div>
                <div>
                    <dt class="font-light uppercase tracking-[0.12em] text-black/35">{"Issued"}</dt>
                    <dd class="mt-1 font-light text-black/65">{ date_label(&document.created_at) }</dd>
                </div>
                <div class="col-span-2">
                    <dt class="font-light uppercase tracking-[0.12em] text-black/35">{"By"}</dt>
                    <dd class="mt-1 font-light text-black/65">
                        { document.issued_by_display_name.as_deref().unwrap_or("—") }
                    </dd>
                </div>
            </dl>

            { download_links(document, true) }
        </article>
    }
}

fn state_pill(state: &str) -> Html {
    let tone = if state == "superseded" {
        "border border-black/10 text-black/35"
    } else {
        "border border-[var(--portal-blue-gray)]/40 text-[var(--portal-navy-soft)]"
    };
    html! {
        <span class={format!(
            "inline-block shrink-0 whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] {tone}"
        )}>
            { state }
        </span>
    }
}

fn download_links(document: &PortalCabinetDocument, mobile: bool) -> Html {
    let pdf_label = if document.signed_artifact_available {
        "Final PDF"
    } else {
        "Issued PDF"
    };
    let pdf_href = format!("/portal/documents/{}/download", document.id);
    let audit_href = format!("/portal/documents/{}/download?artifact=audit", document.id);

    if mobile {
        html! {
            <div class="flex flex-wrap gap-2">
                <a
                    href={pdf_href}
                    class="inline-flex min-h-11 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-archive)]"
                >
                    { pdf_label }
                </a>
                if document.signed_audit_available {
                    <a
                        href={audit_href}
                        class="inline-flex min-h-11 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-archive)]"
                    >
                        {"Audit"}
                    </a>
                }
            </div>
        }
    } else {
        html! {
            <>
                <a
                    href={pdf_href}
                    class="text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] underline underline-offset-4 hover:text-[var(--portal-archive)]"
                >
                    { pdf_label }
                </a>
                if document.signed_audit_available {
                    <a
                        href={audit_href}
                        class="ml-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] underline underline-offset-4 hover:text-[var(--portal-archive)]"
                    >
                        {"Audit"}
                    </a>
                }
            </>
        }
    }
}

fn document_label(document: &PortalCabinetDocument) -> &str {
    document
        .document_type_label
        .as_deref()
        .unwrap_or("Document")
}

fn party_or_deal(document: &PortalCabinetDocument) -> &str {
    document
        .party_name
        .as_deref()
        .or(document.deal_name.as_deref())
        .unwrap_or("—")
}

fn version_label(version: Option<i32>) -> String {
    version
        .map(|version| format!("v{version}"))
        .unwrap_or_else(|| "—".into())
}

fn date_label(value: &str) -> String {
    if value.is_empty() {
        "—".into()
    } else {
        value.chars().take(10).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> PortalCabinetDocument {
        PortalCabinetDocument {
            id: "doc-1".into(),
            deal_name: Some("Villa Mar Azul".into()),
            party_name: Some("Ana Rivera".into()),
            property_name: Some("Dorado".into()),
            document_type_label: Some("Listing Agreement".into()),
            issued_by_display_name: Some("Lisa".into()),
            ..Default::default()
        }
    }

    #[test]
    fn cabinet_search_matches_the_same_metadata_as_the_mature_screen() {
        let documents = vec![document()];
        assert_eq!(visible_documents(&documents, "ana").len(), 1);
        assert_eq!(visible_documents(&documents, "dorado").len(), 1);
        assert_eq!(visible_documents(&documents, "listing").len(), 1);
        assert_eq!(visible_documents(&documents, "lisa").len(), 1);
        assert_eq!(visible_documents(&documents, "missing").len(), 0);
    }

    #[test]
    fn cabinet_date_label_preserves_the_mature_date_contract() {
        assert_eq!(date_label("2026-09-22T18:59:24Z"), "2026-09-22");
        assert_eq!(date_label(""), "—");
    }
}
