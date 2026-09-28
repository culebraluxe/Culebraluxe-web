//! The Documents tab: the project's Vault documents, photos and films, and recording a signed copy.

use super::*;

/// One row of the project's file cabinet: a Vault document, a photograph or a film of the project's property or person.
pub(super) struct ProjectAsset {
    key: String,
    kind: &'static str,
    name: String,
    caption: Option<String>,
    thumbnail: Option<String>,
    href: String,
    type_label: &'static str,
    source: &'static str,
    date: Option<String>,
    /// A document still to be signed: its id, for "Record signed copy".
    signable: Option<String>,
}

/// "Sep 27, 2026" from a stored timestamp, or a dash.
pub(super) fn asset_date(value: Option<&str>) -> String {
    value
        .and_then(|value| value.get(..10))
        .and_then(|day| chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .map(|day| day.format("%b %-d, %Y").to_string())
        .unwrap_or_else(|| "—".into())
}

/// THE PROJECT'S FILE CABINET — its documents, photographs and films, in one read-only list. A view, not a store:
/// Vault documents and the property's media stay where they live. What belongs here is what is linked to the
/// project's property or person. A contract still to be signed can have its signed copy recorded here — the PDF
/// signed by email, until BoldSign is on.
pub(super) fn documents_view(projects: &PortalProjectsPage, project: &PortalProject, on_msg: &Callback<Msg>) -> Html {
    let property_id = project.property_id.as_deref();
    let person_id = project.person_id.as_deref();
    // Signed with the copy still to come: the document is sent and the project's signing step is done.
    let signing_step_done = projects
        .items
        .iter()
        .find(|item| item.project_id.as_deref() == Some(project.id.as_str()) && item.title == "Listing Contract Signed")
        .filter(|item| item.status == "done")
        .map(|item| asset_date(item.planned_start.as_deref()));
    let mut assets: Vec<ProjectAsset> = projects
        .documents
        .iter()
        .filter(|document| {
            (property_id.is_some() && document.property_id.as_deref() == property_id)
                || (person_id.is_some() && document.party_person_id.as_deref() == person_id)
        })
        // The current version of each contract — or, for a contract whose every copy was re-issued away (all
        // superseded), its latest copy, so the contract is never missing from its project.
        .filter(|document| {
            if matches!(document.state.as_str(), "ready" | "sent" | "signed") {
                return true;
            }
            let Some(form) = document.form_instance_id.as_deref() else { return false };
            document.state == "superseded"
                && !projects.documents.iter().any(|other| {
                    other.form_instance_id.as_deref() == Some(form) && matches!(other.state.as_str(), "ready" | "sent" | "signed")
                })
                && !projects.documents.iter().any(|other| {
                    other.form_instance_id.as_deref() == Some(form) && other.state == "superseded" && other.created_at > document.created_at
                })
        })
        .map(|document| {
            let signed = document.state == "signed";
            ProjectAsset {
                key: format!("document:{}", document.id),
                kind: "document",
                name: document.title.clone(),
                caption: Some(match (signed, document.state.as_str(), &signing_step_done) {
                    (true, _, _) => format!("Signed {}", asset_date(document.signed_at.as_deref())),
                    (false, "superseded", _) => "Latest issued copy".to_owned(),
                    (false, "sent", Some(day)) => format!("Signed {day} · copy to come"),
                    _ => "Issued — awaiting signature".to_owned(),
                }),
                thumbnail: None,
                href: format!(
                    "/api/portal/documents/{}/file{}",
                    document.id,
                    if signed && document.signed_artifact_available { "?artifact=signed" } else { "" }
                ),
                type_label: "PDF",
                source: "Vault",
                date: Some(document.created_at.clone()),
                // A superseded copy cannot move to signed in the Vault; only a live one can be recorded.
                signable: (!signed && document.state != "superseded").then(|| document.id.clone()),
            }
        })
        .collect();
    assets.extend(
        projects
            .media
            .iter()
            .filter(|media| Some(media.property_id.as_str()) == property_id)
            .filter_map(|media| match media.media_type.as_str() {
                "image" => Some(ProjectAsset {
                    key: format!("photo:{}", media.id),
                    kind: "photo",
                    name: media.filename.clone().or_else(|| media.alt_text.clone()).unwrap_or_else(|| "Photo".into()),
                    caption: media.caption.clone().or_else(|| media.alt_text.clone()),
                    thumbnail: Some(media.url.clone()),
                    href: media.url.clone(),
                    type_label: "Photo",
                    source: "Property media",
                    date: media.created_at.clone(),
                    signable: None,
                }),
                "video" => media.mux_playback_id.clone().map(|playback| ProjectAsset {
                    key: format!("video:{}", media.id),
                    kind: "video",
                    name: media.caption.clone().unwrap_or_else(|| if media.role == "short" { "Short film".into() } else { "Property film".into() }),
                    caption: None,
                    thumbnail: Some(format!("https://image.mux.com/{playback}/thumbnail.jpg?width=160")),
                    href: format!("https://player.mux.com/{playback}"),
                    type_label: "Video",
                    source: "Mux",
                    date: media.created_at.clone(),
                    signable: None,
                }),
                _ => None,
            }),
    );
    let filter = match projects.documents_filter.as_str() {
        "document" => "document",
        "photo" => "photo",
        "video" => "video",
        _ => "all",
    };
    let count = |kind: &str| assets.iter().filter(|asset| asset.kind == kind).count();
    let (documents, photos, videos) = (count("document"), count("photo"), count("video"));
    let visible: Vec<&ProjectAsset> = assets.iter().filter(|asset| filter == "all" || asset.kind == filter).collect();
    let chip = |key: &'static str, label: &'static str, count: usize| {
        let active = filter == key;
        html! {
            <button type="button" aria-pressed={active.to_string()}
                onclick={on_msg.reform(move |_: MouseEvent| Msg::ProjectDocumentsFilterChanged(key.into()))}
                class={classes!("rounded-full", "px-3", "py-1.5", "text-[11px]", "font-medium", "transition",
                    if active { "bg-white/15 text-white ring-1 ring-inset ring-white/20" } else { "text-white/55 hover:bg-white/[0.07] hover:text-white/90" })}>
                {label} <span class="ml-1 text-[10px] opacity-65">{count}</span>
            </button>
        }
    };
    let columns = "grid grid-cols-[minmax(0,1.7fr)_90px_135px_105px] gap-3";
    let signing = projects.signing_document_id.clone();
    let date_change = on_msg.reform(|event: InputEvent| {
        Msg::ProjectSignedCopyDate(event.target_unchecked_into::<web_sys::HtmlInputElement>().value())
    });
    let file_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let input = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            let file = input.files().and_then(|list| list.get(0));
            input.set_value("");
            if let Some(file) = file {
                on_msg.emit(Msg::ProjectSignedCopyChosen(file));
            }
        })
    };
    html! {
        <section class="flex h-full min-h-0 w-full flex-col overflow-hidden rounded-[var(--portal-tab-radius)] border border-white/20 bg-[color-mix(in_srgb,var(--portal-navy)_94%,transparent)] shadow-sm">
            <div class="flex shrink-0 items-center justify-between gap-3 border-b border-white/10 px-3 py-2">
                <div class="flex items-center gap-1" role="group" aria-label="Asset filter">
                    { chip("all", "All", assets.len()) }
                    { chip("document", "Documents", documents) }
                    { chip("photo", "Photos", photos) }
                    { chip("video", "Videos", videos) }
                </div>
                <span class="hidden text-[10px] font-light uppercase tracking-[0.12em] text-white/35 sm:inline">{"Vault + Property media + Mux · this project's property and person"}</span>
            </div>
            <div class={classes!(columns, "shrink-0", "border-b", "border-white/10", "px-3", "py-2", "text-[9px]", "font-semibold", "uppercase", "tracking-[0.12em]", "text-white/35")}>
                <span>{"Name"}</span><span>{"Type"}</span><span>{"Source"}</span><span>{"Updated"}</span>
            </div>
            <div class="min-h-0 flex-1 overflow-y-auto">
                if visible.is_empty() {
                    <div class="flex h-full min-h-48 items-center justify-center px-6 text-center">
                        <p class="text-[13px] font-light text-white/45">
                            { if assets.is_empty() {
                                if property_id.is_none() && person_id.is_none() { "No property or person is linked to this project yet.".to_owned() }
                                else { "No documents, photos or films are linked to this project's property or person yet.".to_owned() }
                            } else { format!("No {} are linked to this project.", match filter { "photo" => "photos", "video" => "films", _ => "documents" }) } }
                        </p>
                    </div>
                } else {
                    <ul class="divide-y divide-white/[0.08]">
                        { for visible.iter().map(|asset| {
                            let open_signing = asset.signable.is_some() && asset.signable == signing;
                            html! {
                            <li key={asset.key.clone()} class="px-3 py-2.5 transition hover:bg-white/[0.04]">
                                <div class={classes!(columns, "items-center")}>
                                    <a href={asset.href.clone()} target="_blank" rel="noreferrer" class="group block min-w-0" title="Open">
                                        <div class="flex min-w-0 items-center gap-3">
                                            if let Some(thumbnail) = asset.thumbnail.clone() {
                                                <img src={thumbnail} alt={asset.name.clone()} loading="lazy"
                                                    class="h-11 w-14 shrink-0 rounded-md border border-white/15 object-cover" />
                                            } else {
                                                <span class="flex h-11 w-14 shrink-0 items-center justify-center rounded-md border border-white/10 bg-white/[0.06] text-[var(--portal-gold)]">
                                                    { glyph("file-text", "h-5 w-5") }
                                                </span>
                                            }
                                            <span class="min-w-0">
                                                <span class="block truncate text-[14px] font-medium text-white/95">{asset.name.clone()}</span>
                                                if let Some(caption) = asset.caption.clone().filter(|caption| *caption != asset.name) {
                                                    <span class="mt-0.5 block truncate text-[11px] font-light text-white/45">{caption}</span>
                                                }
                                            </span>
                                        </div>
                                    </a>
                                    <span class="text-[12px] font-light text-white/65">{asset.type_label}</span>
                                    <span class="flex items-center gap-2 text-[12px] font-light text-white/65">
                                        {asset.source}
                                        if let Some(document_id) = asset.signable.clone() {
                                            <button type="button"
                                                onclick={on_msg.reform(move |_: MouseEvent| Msg::ProjectSignedCopyStart(document_id.clone()))}
                                                class="rounded-full border border-[var(--portal-gold)]/60 px-2 py-0.5 text-[9px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-gold)] hover:bg-white/10">
                                                { if open_signing { "Cancel" } else { "Record signed" } }
                                            </button>
                                        }
                                    </span>
                                    <span class="text-[11px] font-light text-white/45">{asset_date(asset.date.as_deref())}</span>
                                </div>
                                if open_signing {
                                    <div class="mt-2 flex flex-wrap items-end gap-3 rounded-md border border-white/10 bg-white/[0.05] px-3 py-2">
                                        <label class="text-[10px] font-semibold uppercase tracking-[0.1em] text-white/55">
                                            {"Signed on"}
                                            <input type="date" value={projects.signing_date.clone()} oninput={date_change.clone()} disabled={projects.signing_busy}
                                                class="mt-1 block h-9 rounded-md border border-white/20 bg-white/90 px-2 text-[13px] font-normal normal-case tracking-normal text-[var(--portal-navy)]" />
                                        </label>
                                        <input id="project-signed-copy" type="file" accept="application/pdf" onchange={file_change.clone()} class="hidden" />
                                        <button type="button" disabled={projects.signing_busy}
                                            onclick={Callback::from(|_: MouseEvent| open_signed_copy_picker())}
                                            class="h-9 rounded-md bg-[var(--portal-gold)] px-4 text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-navy)] disabled:opacity-50">
                                            { if projects.signing_busy { "Recording…" } else { "Choose the signed PDF" } }
                                        </button>
                                        <button type="button" disabled={projects.signing_busy}
                                            onclick={on_msg.reform(|_: MouseEvent| Msg::ProjectSignedCopyToCome)}
                                            class="h-9 rounded-md border border-[var(--portal-gold)]/70 px-4 text-[11px] font-semibold uppercase tracking-[0.1em] text-[var(--portal-gold)] hover:bg-white/10 disabled:opacity-50">
                                            {"Mark signed — copy to come"}
                                        </button>
                                        <span class="text-[11px] font-light text-white/50">{"The PDF becomes the executed copy; without it, the contract waits as “copy to come”. Either way the project's signing step is done."}</span>
                                    </div>
                                }
                            </li>
                        }}) }
                    </ul>
                }
            </div>
        </section>
    }
}

/// Opens the hidden PDF picker for "Record signed".
pub(super) fn open_signed_copy_picker() {
    use web_sys::wasm_bindgen::JsCast;
    if let Some(input) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("project-signed-copy"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        input.click();
    }
}
