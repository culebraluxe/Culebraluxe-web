//! Photos and films: the media editor, the pickers, and the chunked and Mux uploaders.

use super::*;

pub(super) fn video_editor(model: &Vm<'_>, property: &PortalOpsProperty, media: &[PortalOpsMediaAsset], on_msg: &Callback<Msg>) -> Html {
    let videos = media
        .iter()
        .filter(|item| item.media_type == "video" && item.mux_playback_id.is_some())
        .collect::<Vec<_>>();
    let films = videos.iter().filter(|item| item.role == "video").count() as i64;
    let shorts = videos.iter().filter(|item| item.role == "short").count() as i64;
    html! {
        <div class="space-y-4">
            {section_intro(
                "Video",
                "Property films stream from Mux. The playback identity stays canonical in Media while the large source file lives at Mux.",
            )}
            <div class="grid gap-3 sm:grid-cols-3">
                {count_card("Videos", property.video_count)}
                {count_card("Property films", films)}
                {count_card("Short films", shorts)}
            </div>
            if let Some(error) = model.error.clone() {
                <div class="rounded-[var(--portal-tab-radius)] border border-red-400/60 bg-red-50 px-3 py-2 text-[12px] font-light text-red-700">
                    {error}
                </div>
            }
            {video_uploader(model, on_msg)}
            // The films this property shows, played by Mux's own player — what a buyer sees on the property page.
            <div class="grid gap-3 lg:grid-cols-2">
                { for videos.iter().map(|video| {
                    let playback = video.mux_playback_id.clone().unwrap_or_default();
                    html! {
                        <figure class="overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-navy)]">
                            <div class="relative aspect-video">
                                <iframe src={format!("https://player.mux.com/{playback}")}
                                    title={video.caption.clone().unwrap_or_else(|| "Property film".into())}
                                    allow="autoplay; fullscreen; picture-in-picture; airplay" allowfullscreen=true
                                    class="absolute inset-0 h-full w-full border-0"></iframe>
                            </div>
                            <figcaption class="flex items-center justify-between gap-2 px-3 py-2 text-[11px] font-light text-white/85">
                                <span class="truncate">{video.caption.clone().unwrap_or_else(|| "Property film".into())}</span>
                                <span class="shrink-0 uppercase tracking-[0.12em] text-white/60">{ if video.role == "short" { "Short" } else { "Film" } }</span>
                            </figcaption>
                        </figure>
                    }
                }) }
            </div>
        </div>
    }
}

/// Choose a film: it goes to Mux in pieces, straight from the browser, and appears here when Mux has it ready.
pub(super) fn video_uploader(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let file_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            if let Some(file) = crate::app::exec::take_files(&event).into_iter().next() {
                on_msg.emit(Msg::VideoChosen(file));
            }
        })
    };
    let role_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            on_msg.emit(Msg::VideoRoleChanged(crate::app::exec::select_value(&event)))
        })
    };
    let caption_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            on_msg.emit(Msg::VideoCaptionChanged(crate::app::exec::input_value(&event)))
        })
    };
    let busy = model.ops.video_file_name.is_some();
    let status = match &model.ops.video_progress {
        Some((sent, total, stage)) if stage == "preparing" => {
            let _ = (sent, total);
            "Uploaded — Mux is preparing the video…".to_owned()
        }
        Some((sent, total, _)) => format!(
            "Uploading {:.0}% ({:.0} of {:.0} MB)",
            if *total > 0.0 { sent / total * 100.0 } else { 0.0 },
            sent / 1_048_576.0,
            total / 1_048_576.0
        ),
        None => "Choose a video — it uploads straight to Mux, and resumes if it is interrupted".to_owned(),
    };
    let percent = model
        .ops
        .video_progress
        .as_ref()
        .map(|(sent, total, stage)| if stage == "preparing" || *total <= 0.0 { 100.0 } else { sent / total * 100.0 })
        .unwrap_or(0.0);
    html! {
        <div class="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/45 p-3">
            <input id="ops-video-file" type="file" accept="video/*" onchange={file_change} class="hidden" />
            <div class="flex flex-wrap items-center gap-3">
                <span class="min-w-0 flex-1 truncate text-[11px] font-light text-black/55">
                    { match &model.ops.video_file_name { Some(name) => format!("{name} — {status}"), None => status.clone() } }
                </span>
                <button
                    type="button"
                    disabled={busy}
                    onclick={Callback::from(|_: MouseEvent| open_picker("ops-video-file"))}
                    class="inline-flex h-9 shrink-0 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-40"
                >
                    {"Add video"}
                </button>
            </div>
            if busy {
                <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-[var(--portal-navy)]/10">
                    <div class="h-full bg-[var(--portal-gold-muted)] transition-all" style={format!("width: {percent:.1}%")}></div>
                </div>
            }
            <div class="mt-2 grid gap-2 sm:grid-cols-[12rem_minmax(0,1fr)]">
                <label class="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                    {"Kind"}
                    <select onchange={role_change} disabled={busy}
                        class="mt-1 block h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/80 px-3 text-[13px] font-light normal-case tracking-normal text-[var(--portal-navy)]">
                        <option value="video" selected={model.ops.video_role != "short"}>{"Property film"}</option>
                        <option value="short" selected={model.ops.video_role == "short"}>{"Short film"}</option>
                    </select>
                </label>
                <label class="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                    {"Caption"}
                    <input value={model.ops.video_caption.clone()} oninput={caption_change} disabled={busy}
                        placeholder="Sunset over Flamenco from the terrace"
                        class="mt-1 block h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/80 px-3 text-[13px] font-light normal-case tracking-normal text-[var(--portal-navy)]" />
                </label>
            </div>
        </div>
    }
}

pub(super) fn media_editor(
    model: &Vm<'_>,
    property: &PortalOpsProperty,
    media: &[PortalOpsMediaAsset],
    on_msg: &Callback<Msg>,
) -> Html {
    let mut images = media
        .iter()
        .filter(|item| item.media_type == "image")
        .collect::<Vec<_>>();
    images.sort_by_key(|item| if item.role == "hero" { 0 } else { 1 });

    let active_index = if images.is_empty() {
        0
    } else {
        model.ops.media_index.min(images.len() - 1)
    };
    let active = images.get(active_index).copied();
    let previous_index = if images.is_empty() {
        0
    } else {
        (active_index + images.len() - 1) % images.len()
    };
    let next_index = if images.is_empty() {
        0
    } else {
        (active_index + 1) % images.len()
    };

    let previous = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsMediaSelected(previous_index)))
    };
    let next = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsMediaSelected(next_index)))
    };
    let file_change = media_file_change(on_msg);

    html! {
        // `min-w-0` IS THE FIX FOR "IT GOES PAST THE LEFT WIDGET". This column is a flex/grid child, and such a child
        // defaults to `min-width: auto` — it refuses to shrink below its content's intrinsic width. A 6000px
        // photograph therefore widened the column past the sidebar instead of being scaled into it. With `min-w-0`
        // the column can be no wider than the space it was given, and `max-w-full` keeps the media inside it.
        <div class="min-w-0 max-w-full space-y-4">
            // NO SECTION INTRO HERE. It said "Review the Property photography in-place, then add the next photo
            // without leaving the canonical record" — a sentence that describes the screen to someone who has not
            // seen it, placed on a screen only reached by someone who already knows why they are there. The tab is
            // called Photos.

            // THE FAILURE HAS TO BE VISIBLE. Every op in this screen writes its problem into `model.error`, and this
            // view never rendered it — so an upload that failed said nothing at all. "Nothing happened" is the most
            // expensive bug report there is: it describes a screen, not a cause.
            if let Some(error) = model.error.clone().filter(|_| model.ops.section == "photos") {
                <div class="rounded-[var(--portal-tab-radius)] border border-red-400/60 bg-red-50 px-3 py-2 text-[12px] font-light text-red-700">
                    {error}
                </div>
            }

            // NO COUNT STRIP HERE. It printed "6 photos · 0 hero · 0 documents" one line above the uploader while the
            // Property header already carried "MEDIA — 6 images": the same number, twice, and the second copy was the
            // one in the reading path. The counts live in the header (Media, Hero, Documents) — once each. This tab
            // is for the photographs.

            // THE UPLOADER IS ALWAYS HERE, not behind a toggle. It used to need a "+ Add new photo" click to appear,
            // which made a second control for a job the panel's own button already does. One screen, one upload
            // button: the one inside this panel.
            {ops_media_uploader(model, on_msg)}

            <section class="overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-navy)]">
                if let Some(image) = active {
                    // A MEDIA MANAGER, NOT A CAROUSEL.
                    //
                    // This was one photograph stretched edge to edge and cropped to fill a full-width band — which
                    // misrepresents the picture (a portrait shot became a slice of its middle) and made the panel's
                    // height jump between photographs. Now the selected photograph is large on the LEFT, contained
                    // (`object-contain`) so its proportions are its own, in a box of stable height; and every uploaded
                    // photograph is a thumbnail on the RIGHT. The bottom filmstrip is gone — it duplicated that grid.
                    <div class="grid min-w-0 gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,190px)]">
                        <div class="min-w-0">
                    <div class="relative h-[300px] w-full max-w-full overflow-hidden rounded-[var(--portal-tab-radius)] bg-black/85 sm:h-[380px] xl:h-[440px]">
                        <img
                            src={image.url.clone()}
                            alt={image.alt_text.clone().unwrap_or_else(|| property.name.clone())}
                            class="h-full w-full object-contain"
                        />
                        <div class="pointer-events-none absolute inset-0 bg-gradient-to-t from-black/50 via-transparent to-black/10"></div>
                        <div class="absolute left-4 top-4 flex gap-2">
                            if image.role == "hero" {
                                <span class="rounded-full border border-white/35 bg-black/30 px-2.5 py-1 text-[9px] font-semibold uppercase tracking-[0.13em] text-white backdrop-blur-sm">
                                    {"Hero"}
                                </span>
                            }
                            <span class="rounded-full border border-white/30 bg-black/25 px-2.5 py-1 text-[9px] font-medium text-white/90 backdrop-blur-sm">
                                {format!("{} / {}", active_index + 1, images.len())}
                            </span>
                        </div>
                        // What can be done to this photo: make it the hero, or delete it. Delete asks once — the second
                        // press, on the same photo, deletes it and its web copies.
                        <div class="absolute right-4 top-4 z-10 flex gap-2">
                            if image.role != "hero" {
                                <button type="button"
                                    onclick={ { let id = image.id.clone(); on_msg.reform(move |_: MouseEvent| Msg::MakeHero(id.clone())) } }
                                    class="rounded-full border border-white/40 bg-black/40 px-3 py-1 text-[9px] font-semibold uppercase tracking-[0.13em] text-white backdrop-blur-sm hover:bg-black/60">
                                    {"Make hero"}
                                </button>
                            }
                            <button type="button"
                                onclick={ { let id = image.id.clone(); on_msg.reform(move |_: MouseEvent| Msg::DeletePhoto(id.clone())) } }
                                class={classes!(
                                    "rounded-full", "border", "px-3", "py-1", "text-[9px]", "font-semibold", "uppercase", "tracking-[0.13em]", "text-white", "backdrop-blur-sm",
                                    if model.ops.media_confirm_delete.as_deref() == Some(image.id.as_str()) {
                                        "border-red-300 bg-red-600/85 hover:bg-red-600"
                                    } else {
                                        "border-white/40 bg-black/40 hover:bg-black/60"
                                    }
                                )}>
                                { if model.ops.media_confirm_delete.as_deref() == Some(image.id.as_str()) { "Confirm delete" } else { "Delete" } }
                            </button>
                        </div>
                        if images.len() > 1 {
                            <button
                                type="button"
                                onclick={previous}
                                aria-label="Previous photo"
                                class="absolute left-3 top-1/2 grid h-11 w-11 -translate-y-1/2 place-items-center rounded-full border border-white/35 bg-black/30 text-xl text-white backdrop-blur-md hover:bg-black/55"
                            >
                                {"‹"}
                            </button>
                            <button
                                type="button"
                                onclick={next}
                                aria-label="Next photo"
                                class="absolute right-3 top-1/2 grid h-11 w-11 -translate-y-1/2 place-items-center rounded-full border border-white/35 bg-black/30 text-xl text-white backdrop-blur-md hover:bg-black/55"
                            >
                                {"›"}
                            </button>
                        }
                        <div class="absolute inset-x-0 bottom-0 p-4 text-white">
                            <div class="text-[11px] font-medium">{image.filename.clone().unwrap_or_else(|| property.name.clone())}</div>
                            if let Some(caption) = image.caption.as_deref() {
                                <div class="mt-1 text-[10px] font-light text-white/70">{caption}</div>
                            }
                        </div>
                    </div>

                        </div>

                        // THE UPLOADED PHOTOGRAPHS, BESIDE THE SELECTED ONE. Two columns where there is room, one
                        // where there is not, scrolling inside the panel — a media manager instead of a strip that
                        // ran off the bottom and capped itself at eight. `object-contain` on a fixed-ratio frame,
                        // because a thumbnail that crops or stretches is a thumbnail you cannot judge.
                        <div class="min-w-0">
                            <div class="grid max-h-[440px] grid-cols-2 gap-1.5 overflow-y-auto pr-0.5 max-lg:grid-cols-3 max-sm:grid-cols-2">
                                {for images.iter().enumerate().map(|(index, image)| {
                                    let onclick = {
                                        let on_msg = on_msg.clone();
                                        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::OpsMediaSelected(index)))
                                    };
                                    html! {
                                        <button
                                            type="button"
                                            {onclick}
                                            aria-current={(index == active_index).to_string()}
                                            class={classes!(
                                                "relative","aspect-[4/3]","w-full","overflow-hidden","rounded-[var(--portal-tab-radius)]","border","bg-black/70","transition",
                                                if index == active_index { "border-[var(--portal-gold)] opacity-100" } else { "border-transparent opacity-65 hover:opacity-100" }
                                            )}
                                        >
                                            <img src={image.url.clone()} alt="" class="h-full w-full object-contain" />
                                            if image.role == "hero" {
                                                <span class="absolute left-1 top-1 rounded-full bg-black/55 px-1.5 py-0.5 text-[8px] font-semibold uppercase tracking-[0.1em] text-white">
                                                    {"Hero"}
                                                </span>
                                            }
                                        </button>
                                    }
                                })}
                            </div>
                        </div>
                    </div>
                } else {
                    <div class="grid h-[300px] place-items-center p-8 text-center sm:h-[390px]">
                        <div>
                            <div class="font-serif text-2xl font-light text-white">{"No photos yet"}</div>
                            <p class="mt-2 text-[12px] font-light text-white/55">{"Add the first Property image below."}</p>
                        </div>
                    </div>
                }
            </section>

            // THE FILE INPUT LIVES HERE, ALWAYS, HIDDEN — not inside the uploader panel. A file input can only be
            // opened from a user gesture, so an input that only exists once the panel is open cannot be opened BY
            // the click that opens that panel: the input would be created after the gesture that needed it. Keeping
            // it in the tree lets "+ Add new photo" open the picker directly.
            <input
                id="ops-media-file"
                type="file"
                accept="image/*"
                multiple=true
                onchange={file_change}
                class="hidden"
            />
            // A whole folder: every photo in it uploads, one after another.
            <input
                id="ops-media-folder"
                type="file"
                webkitdirectory=true
                multiple=true
                onchange={media_file_change(on_msg)}
                class="hidden"
            />

            // NO SECOND BUTTON HERE. This row carried a caption and a "+ Add new photo" / "Close uploader" button —
            // a duplicate of the control inside the uploader panel, which is the one upload button on this screen. A
            // row whose only job is to duplicate a control above it is how a screen ends up with three buttons for
            // one action, and the extra two are what made this look broken.
        </div>
    }
}

/// Opens the listing-media file input — the hidden one that lives in the Photos tab at all times.
///
/// A file picker may only be opened from a USER GESTURE, so this is called from inside the click handler rather than
/// from an effect afterwards: by the time an effect runs, the gesture is over and the browser refuses to open it. That
/// is also why the input is not created by the button — an element created by a click cannot be clicked by the same
/// click.
///
/// If the element is missing this does nothing at all, deliberately: the uploader's own "Choose file" button is the
/// fallback, so a lookup that fails leaves a usable screen rather than a panic.
pub(super) fn open_file_picker() {
    open_picker("ops-media-file");
}

/// Opens the folder picker (every photo in the chosen folder uploads).
pub(super) fn open_folder_picker() {
    open_picker("ops-media-folder");
}

pub(super) fn open_picker(id: &str) {
    crate::app::exec::open_file_picker(id);
}

/// The chosen-file callback, shared by the hidden input and the uploader panel so both write the same message field.
pub(super) fn media_file_change(on_msg: &Callback<Msg>) -> Callback<Event> {
    let on_msg = on_msg.clone();
    Callback::from(move |event: Event| {
        // Cleared by `take_files`, so choosing the same files again (after a failure) is a new choice.
        on_msg.emit(Msg::OpsMediaFilesChosen(crate::app::exec::take_files(&event)));
    })
}

pub(super) fn ops_media_uploader(model: &Vm<'_>, on_msg: &Callback<Msg>) -> Html {
    let role_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let value = crate::app::exec::select_value(&event);
            on_msg.emit(Msg::OpsMediaRoleChanged(value));
        })
    };
    let alt_change = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let value = crate::app::exec::input_value(&event);
            on_msg.emit(Msg::OpsMediaAltChanged(value));
        })
    };
    let choose_again = Callback::from(move |_: MouseEvent| open_file_picker());
    let choose_folder = Callback::from(move |_: MouseEvent| open_folder_picker());
    let batch = (model.ops.media_batch_total > 1).then(|| {
        let at = (model.ops.media_batch_done + model.ops.media_batch_failed.len() + 1).min(model.ops.media_batch_total);
        format!("Uploading {at} of {}…", model.ops.media_batch_total)
    });

    html! {
        <section class="min-w-0 max-w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-3">
            // THE BUTTON IS FIRST, AND THE FIELDS FOLLOW — the ordering is the point.
            //
            // This panel is why the screen felt broken: it appeared only after a click, then rendered under the
            // gallery, so the one control that does the job sat below the fold beneath two fields that describe a file
            // which does not exist yet. Now the button is the first thing in the panel, so it is on screen the moment
            // the tab opens, and Role and Alt text — which only describe a photograph that has already been chosen —
            // come after it.
            <div class="flex flex-wrap items-center justify-between gap-3">
                <span class="min-w-0 flex-1 truncate text-[11px] font-light text-black/45">
                    {model.ops.media_file_name.clone().unwrap_or_else(|| "Choose photos or a whole folder — the upload starts by itself".into())}
                </span>
                // RIGHT-ALIGNED, because every other action on this screen is. A single button sitting on the left while
                // Save, Revert and the rest sit on the right reads as a different kind of control than it is.
                if model.ops.media_uploading {
                    <span class="shrink-0 text-[11px] font-light text-[var(--portal-gold-muted)]">{ batch.clone().unwrap_or_else(|| "Uploading…".into()) }</span>
                }
                <button
                    type="button"
                    onclick={choose_folder}
                    disabled={model.ops.media_uploading}
                    class="inline-flex h-9 shrink-0 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-navy)] px-4 text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-40"
                >
                    {"Add folder"}
                </button>
                <button
                    type="button"
                    onclick={choose_again}
                    class="inline-flex h-9 shrink-0 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-semibold uppercase tracking-[0.12em] text-white"
                >
                    {"Add photos"}
                </button>
            </div>
            <div class="mt-2 grid gap-2 sm:grid-cols-2">
                <label class="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                    {"Image role"}
                    <select
                        value={model.ops.media_role.clone()}
                        onchange={role_change}
                        class="mt-1 h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light"
                    >
                        <option value="gallery" selected={model.ops.media_role == "gallery"}>{"Gallery"}</option>
                        <option value="hero" selected={model.ops.media_role == "hero"}>{"Hero"}</option>
                    </select>
                </label>
                <label class="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
                    {"Alt text"}
                    <input
                        value={model.ops.media_alt.clone()}
                        oninput={alt_change}
                        placeholder="Oceanfront villa overlooking Culebra"
                        class="mt-1 h-9 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light"
                    />
                </label>
            </div>
        </section>
    }
}
