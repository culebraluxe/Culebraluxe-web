use yew::prelude::*;

use crate::model::{Msg, PortalCabinetDocument, PortalCabinetPage};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct CabinetProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct Cabinet;

impl Component for Cabinet {
    type Message = ();
    type Properties = CabinetProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen = crate::model::screen("cabinet").expect("cabinet screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { cabinet(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

fn payload(model: &crate::model::Model) -> Option<&PortalCabinetPage> {
    model
        .page
        .as_ref()
        .and_then(|page| page.portal.as_ref())
        .and_then(|portal| portal.cabinet.as_ref())
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

fn cabinet(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let Some(data) = payload(model) else {
        return html! {
            <section class="portal-glass-panel rounded-[var(--portal-panel-radius)] p-6">
                <p class="text-sm font-light text-black/45">
                    { if model.loading { "Loading Cabinet…" } else { "Cabinet data is not available." } }
                </p>
            </section>
        };
    };

    let visible = visible_documents(&data.documents, &model.controls.query);
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

                    <label class="w-full max-w-xs">
                        <span class="mb-1 block text-[10px] font-light uppercase tracking-[0.16em] text-black/40">
                            {"Filter by deal / client / property"}
                        </span>
                        <input
                            type="search"
                            {oninput}
                            value={model.controls.query.clone()}
                            placeholder="Search…"
                            class="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-3 py-2 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
                        />
                    </label>
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
        <span class={classes!(
            "inline-block",
            "shrink-0",
            "whitespace-nowrap",
            "rounded-full",
            "px-3",
            "py-1",
            "text-[10px]",
            "font-light",
            "uppercase",
            "tracking-[0.14em]",
            tone,
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
