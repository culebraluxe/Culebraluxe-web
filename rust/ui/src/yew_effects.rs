//! The effects, run by Yew.
//!
//! The reducer decides what must happen; this module only performs those effects and returns completion messages.

use gloo_net::http::Request;
use wasm_bindgen::JsCast;
use yew::platform::spawn_local;
use yew::Callback;

use crate::model::{
    Effect, Msg, PortalDealPeopleSearch, PortalEntitlements, PortalRoleEntitlements,
    PortalSecurityUser, PropertyRecent,
};

const PAGE_PATH: &str = "/api/rust-ui/public-page";
const ROWS_PATH: &str = "/api/rust-ui/public-rows";
const PORTAL_PATH: &str = "/api/portal/rust-ui/page";
const ROLE_ENTITLEMENTS_PATH: &str = "/api/portal/rust-ui/role-entitlements";
const SECURITY_USERS_PATH: &str = "/api/portal/rust-ui/security-users";
const TECH_PATH: &str = "/api/portal/rust-ui/tech";
const FLIGHT_RECORDER_PATH: &str = "/api/portal/flight-recorder";
const COCKPIT_PATH: &str = "/api/portal/rust-ui/cockpit";
const CABINET_PATH: &str = "/api/portal/rust-ui/cabinet";
const CLIENTS_PATH: &str = "/api/portal/rust-ui/clients";
const FORMS_PATH: &str = "/api/portal/rust-ui/forms";
const PROJECTS_PATH: &str = "/api/portal/rust-ui/projects";
const DEALS_PATH: &str = "/api/portal/rust-ui/deals";
const ACCOUNTING_PATH: &str = "/api/portal/rust-ui/accounting";
const OPPS_PATH: &str = "/api/portal/rust-ui/opps";
const RECORDS_PATH: &str = "/api/portal/rust-ui/records";
const LISTING_MEDIA_PATH: &str = "/api/portal/rust-ui/listing-media";
const LISTING_MEDIA_UPLOAD_PATH: &str = "/api/property-media/upload";
/// The chunked path, for photographs too large for one request. The browser cannot hold the Rust API's bridge key, so
/// every step goes through the server route, which resolves the acting user and requires `listing.write` first.
const OPS_MEDIA_CHUNKED_PATH: &str = "/api/property-media/chunked";

pub fn run(effect: Effect, dispatch: &Callback<Msg>) {
    match effect {
        Effect::FetchEntitlements { generation } => {
            fetch_entitlements(generation, dispatch);
        }
        Effect::SetRoleEntitlement {
            generation,
            role_code,
            action,
            granted,
        } => {
            set_role_entitlement(generation, role_code, action, granted, dispatch);
        }
        Effect::SetUserPrimaryRole {
            generation,
            app_user_id,
            role_code,
        } => {
            set_user_primary_role(generation, app_user_id, role_code, dispatch);
        }
        Effect::PropertyBrowserRead {
            id,
            slug,
            title,
            valid_slugs,
        } => {
            read_property_browser(id, slug, title, valid_slugs, dispatch);
        }
        Effect::PropertyFavoriteWrite {
            id,
            slug,
            title,
            saved,
        } => {
            write_property_favorite(id, slug, title, saved, dispatch);
        }
        Effect::SubmitContact {
            submission,
            submission_id,
        } => {
            submit_contact(submission, submission_id, dispatch);
        }
        Effect::BuyerToolsRead => {
            let read = |key: &str| {
                browser_storage()
                    .and_then(|storage| storage.get_item(key).ok().flatten())
                    .unwrap_or_default()
            };
            // Each entry is read on its own, so one malformed record drops that record, not the list.
            let compare = parse_entries::<crate::search::CompareEntry>(&read(COMPARE_KEY));
            let searches = parse_entries::<crate::search::SavedSearch>(&read(SAVED_SEARCHES_KEY));
            dispatch.emit(Msg::BuyerToolsLoaded { compare, searches });
        }
        Effect::CompareWrite(entries) => {
            write_store(COMPARE_KEY, &entries, "culebraluxe:compare-changed");
        }
        Effect::SavedSearchesWrite(searches) => {
            write_store(
                SAVED_SEARCHES_KEY,
                &searches,
                "culebraluxe:saved-searches-changed",
            );
        }
        Effect::ListingFavoritesRead => {
            let ids = browser_storage()
                .map(|storage| {
                    favorite_entries(&storage)
                        .iter()
                        .filter_map(favorite_id)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            dispatch.emit(Msg::ListingFavoritesLoaded(ids));
        }
        Effect::FetchFlightRecorder {
            screen,
            instance_id,
            generation,
        } => {
            run_flight_recorder_read(screen, generation, instance_id, dispatch);
        }
        Effect::TechCommand {
            screen,
            generation,
            body,
        } => {
            run_tech_command(screen, generation, body, dispatch);
        }
        Effect::CompleteCockpitTask {
            screen,
            generation,
            task_id,
        } => {
            run_cockpit_command(screen, generation, task_id, dispatch);
        }
        Effect::AccountingCommand {
            screen,
            generation,
            body,
        } => {
            run_accounting_command(screen, generation, body, dispatch);
        }
        Effect::RecordArchive {
            screen,
            property_id,
            archived,
            search,
            page,
            generation,
        } => {
            run_record_archive(
                screen,
                generation,
                property_id,
                archived,
                search,
                page,
                dispatch,
            );
        }
        Effect::UploadListingMedia {
            screen,
            property_id,
            role,
            alt,
            generation,
        } => {
            run_listing_media_upload(screen, generation, property_id, role, alt, dispatch);
        }
        Effect::UpdateProjectStatus {
            screen,
            generation,
            project_id,
            status,
        } => {
            run_projects_command(
                screen,
                generation,
                serde_json::json!({
                    "action": "projectStatus",
                    "projectId": project_id,
                    "status": status,
                }),
                dispatch,
            );
        }
        Effect::SaveProjectWork {
            screen,
            generation,
            item_id,
            title,
            notes,
            status,
            due_at,
            owner,
        } => {
            run_projects_command(
                screen,
                generation,
                serde_json::json!({
                    "action": "wbsSave",
                    "itemId": item_id,
                    "title": title,
                    "notes": notes,
                    "status": status,
                    "dueAt": due_at,
                    "owner": owner,
                }),
                dispatch,
            );
        }
        Effect::BrowserNavigate { href } => {
            if let Some(window) = web_sys::window() {
                let _ = window.location().set_href(&href);
            }
        }
        Effect::SearchDealPeople {
            screen,
            generation,
            query,
        } => {
            run_deal_people_search(screen, generation, query, dispatch);
        }
        Effect::CreateDeal {
            screen,
            generation,
            property_id,
            client_person_id,
            owner_user_id,
            notes,
        } => {
            run_deal_create(
                screen,
                generation,
                property_id,
                client_person_id,
                owner_user_id,
                notes,
                dispatch,
            );
        }
        Effect::SearchDealWorkspacePeople {
            screen,
            generation,
            purpose,
            query,
        } => {
            run_deal_workspace_people_search(screen, generation, purpose, query, dispatch);
        }
        Effect::RunDealWorkspaceCommand {
            screen,
            generation,
            deal_id,
            command,
        } => {
            run_deal_workspace_command(screen, generation, deal_id, command, dispatch);
        }

        Effect::SearchOpsPeople {
            screen,
            query,
            generation,
        } => {
            run_ops_people_search(screen, generation, query, dispatch);
        }
        Effect::UploadOpsMedia {
            screen,
            property_id,
            role,
            alt,
            generation,
        } => {
            run_ops_media_upload(screen, generation, property_id, role, alt, dispatch);
        }
        Effect::SaveOps {
            screen,
            entity,
            id,
            fields,
            search,
            page,
            generation,
        } => {
            run_ops_command(
                screen,
                generation,
                serde_json::json!({
                    "action": "save",
                    "entity": entity,
                    "id": id,
                    "fields": fields,
                    "search": search,
                    "page": page,
                }),
                dispatch,
            );
        }
        Effect::CreateOpsProperty {
            property_type,
            screen,
            name,
            generation,
        } => {
            run_ops_command(
                screen,
                generation,
                serde_json::json!({
                    "action": "createProperty",
                    "name": name,
                    "propertyType": property_type,
                }),
                dispatch,
            );
        }
        Effect::SaveForm {
            screen,
            generation,
            form_id,
            field_values,
            sections,
        } => {
            let dispatch = dispatch.clone();
            spawn_local(async move {
                let body = serde_json::json!({
                    "action": "save",
                    "formId": form_id,
                    "fieldValues": field_values,
                    "sections": sections,
                })
                .to_string();
                let request = match Request::post(FORMS_PATH)
                    .header("content-type", "application/json")
                    .body(body)
                {
                    Ok(request) => request,
                    Err(error) => {
                        dispatch.emit(Msg::EffectFailed {
                            screen: screen.to_string(),
                            generation,
                            message: format!("the form save request could not be built: {error}"),
                        });
                        return;
                    }
                };
                let msg = match request.send().await {
                    Ok(response) if response.ok() => match response.text().await {
                        Ok(body) => Msg::portal_loaded_json(screen, generation, &body),
                        Err(error) => Msg::EffectFailed {
                            screen: screen.to_string(),
                            generation,
                            message: format!("the form save answer could not be read: {error}"),
                        },
                    },
                    Ok(response) => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: format!("the form save failed with {}", response.status()),
                    },
                    Err(error) => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: format!("the form save request could not be sent: {error}"),
                    },
                };
                dispatch.emit(msg);
            });
        }
        Effect::CreateForm {
            screen,
            generation,
            template_id,
            deal_id,
            person_id,
            property_id,
        } => {
            let dispatch = dispatch.clone();
            spawn_local(async move {
                let body = serde_json::json!({
                    "action": "create",
                    "templateId": template_id,
                    "dealId": deal_id,
                    "personId": person_id,
                    "propertyId": property_id,
                })
                .to_string();
                let request = match Request::post(FORMS_PATH)
                    .header("content-type", "application/json")
                    .body(body)
                {
                    Ok(request) => request,
                    Err(error) => {
                        dispatch.emit(Msg::EffectFailed {
                            screen: screen.to_string(),
                            generation,
                            message: format!("the new-form request could not be built: {error}"),
                        });
                        return;
                    }
                };
                let msg = match request.send().await {
                    Ok(response) if response.ok() => match response.text().await {
                        Ok(body) => match serde_json::from_str::<serde_json::Value>(&body)
                            .ok()
                            .and_then(|value| {
                                value
                                    .get("formId")
                                    .and_then(serde_json::Value::as_str)
                                    .map(str::to_owned)
                            }) {
                            Some(form_id) => Msg::FormCreated { form_id },
                            None => Msg::EffectFailed {
                                screen: screen.to_string(),
                                generation,
                                message: "the new-form answer did not contain formId".into(),
                            },
                        },
                        Err(error) => Msg::EffectFailed {
                            screen: screen.to_string(),
                            generation,
                            message: format!("the new-form answer could not be read: {error}"),
                        },
                    },
                    Ok(response) => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: format!("the new-form request failed with {}", response.status()),
                    },
                    Err(error) => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: format!("the new-form request could not be sent: {error}"),
                    },
                };
                dispatch.emit(msg);
            });
        }
        effect => run_read(effect, dispatch),
    }
}

fn run_ops_command(
    screen: &'static str,
    generation: u64,
    body: serde_json::Value,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let request = match Request::post(OPPS_PATH)
            .header("content-type", "application/json")
            .body(body.to_string())
        {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the OPPS workbench command could not be built: {error}"),
                });
                return;
            }
        };

        let msg = match request.send().await {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                if status >= 200 && status < 300 {
                    Msg::portal_loaded_json(screen, generation, &body)
                } else {
                    Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: bridge_error_message(&body, status),
                    }
                }
            }
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the OPPS workbench command could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_ops_people_search(
    screen: &'static str,
    generation: u64,
    query_value: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let url = format!(
            "{DEALS_PATH}?peopleSearch={}",
            encode_component(&query_value)
        );
        let answer = Request::get(&url).send().await;
        let msg = match answer {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => match serde_json::from_str::<PortalDealPeopleSearch>(&body) {
                    Ok(payload) => Msg::OpsPeopleLoaded {
                        screen: screen.to_string(),
                        generation,
                        query: query_value,
                        people: payload.people,
                    },
                    Err(error) => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: format!(
                            "the OPPS Person search answer could not be read: {error}"
                        ),
                    },
                },
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the OPPS Person search answer could not be read: {error}"),
                },
            },
            Ok(response) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the OPPS Person search failed with {}", response.status()),
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the OPPS Person search could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_ops_media_upload(
    screen: &'static str,
    generation: u64,
    property_id: String,
    role: String,
    alt: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let fail = |message: String| Msg::EffectFailed {
            screen: screen.to_string(),
            generation,
            message,
        };

        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            dispatch.emit(fail("the browser document is unavailable.".into()));
            return;
        };
        let Some(element) = document.get_element_by_id("ops-media-file") else {
            dispatch.emit(fail("the OPPS Media file input is unavailable.".into()));
            return;
        };
        let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>() else {
            dispatch.emit(fail(
                "the OPPS Media file input has the wrong element type.".into(),
            ));
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            dispatch.emit(fail("Choose an image before uploading.".into()));
            return;
        };

        // 3 MB per request: comfortably under the gateway's ~4.5 MB cap, so no single request can reach the ceiling
        // that made a 13 MB photograph impossible to upload. The container reassembles the pieces.
        const CHUNK_BYTES: f64 = 3.0 * 1024.0 * 1024.0;
        let size = file.size();
        let chunk_count = ((size / CHUNK_BYTES).ceil() as i32).max(1);

        // STEP ONE — declare the file. Everything the container needs to check the bytes later is settled here, while
        // `media` still has no row to point at.
        let init = match web_sys::FormData::new() {
            Ok(form) => form,
            Err(_) => {
                dispatch.emit(fail("the browser could not create the upload form.".into()));
                return;
            }
        };
        if init.append_with_str("step", "init").is_err()
            || init.append_with_str("propertyId", &property_id).is_err()
            || init.append_with_str("role", &role).is_err()
            || init.append_with_str("filename", &file.name()).is_err()
            || init.append_with_str("mimeType", &file.type_()).is_err()
            || init
                .append_with_str("byteSize", &format!("{size}"))
                .is_err()
            || init
                .append_with_str("chunkCount", &chunk_count.to_string())
                .is_err()
            || init
                .append_with_str("chunkSize", &format!("{CHUNK_BYTES}"))
                .is_err()
            || (!alt.trim().is_empty() && init.append_with_str("altText", alt.trim()).is_err())
        {
            dispatch.emit(fail(
                "the browser could not prepare the selected image.".into(),
            ));
            return;
        }

        let request = match Request::post(OPS_MEDIA_CHUNKED_PATH).body(init) {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(fail(format!("the upload could not be built: {error}")));
                return;
            }
        };
        let upload_id = match request.send().await {
            Ok(response) if response.ok() => {
                let body = response.text().await.unwrap_or_default();
                match serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("uploadId")
                            .and_then(|id| id.as_str())
                            .map(|id| id.to_string())
                    }) {
                    Some(upload_id) => upload_id,
                    None => {
                        dispatch.emit(fail(
                            "the upload was opened but no upload id came back.".into(),
                        ));
                        return;
                    }
                }
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                dispatch.emit(fail(bridge_error_message(&body, status)));
                return;
            }
            Err(error) => {
                dispatch.emit(fail(format!(
                    "the OPPS Media upload could not be started: {error}"
                )));
                return;
            }
        };

        // STEP TWO — the pieces, in order. Sequential on purpose: the container refuses a chunk index beyond the
        // declared count, so racing them would turn a retry into an error.
        let mut index = 0;
        while index < chunk_count {
            let start = index as f64 * CHUNK_BYTES;
            let end = (start + CHUNK_BYTES).min(size);
            let chunk = match file
                .unchecked_ref::<web_sys::Blob>()
                .slice_with_f64_and_f64(start, end)
            {
                Ok(chunk) => chunk,
                Err(_) => {
                    dispatch.emit(fail(format!(
                        "part {} of {chunk_count} could not be read.",
                        index + 1
                    )));
                    return;
                }
            };

            let form = match web_sys::FormData::new() {
                Ok(form) => form,
                Err(_) => {
                    dispatch.emit(fail("the browser could not create the upload form.".into()));
                    return;
                }
            };
            if form.append_with_str("step", "chunk").is_err()
                || form.append_with_str("propertyId", &property_id).is_err()
                || form.append_with_str("uploadId", &upload_id).is_err()
                || form
                    .append_with_str("chunkIndex", &index.to_string())
                    .is_err()
                || form
                    .append_with_blob_and_filename("chunk", &chunk, &file.name())
                    .is_err()
            {
                dispatch.emit(fail(format!(
                    "part {} of {chunk_count} could not be prepared.",
                    index + 1
                )));
                return;
            }

            let request = match Request::post(OPS_MEDIA_CHUNKED_PATH).body(form) {
                Ok(request) => request,
                Err(error) => {
                    dispatch.emit(fail(format!(
                        "part {} of {chunk_count} could not be built: {error}",
                        index + 1
                    )));
                    return;
                }
            };
            match request.send().await {
                Ok(response) if response.ok() => {}
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    dispatch.emit(fail(format!(
                        "part {} of {chunk_count} was refused: {}",
                        index + 1,
                        bridge_error_message(&body, status)
                    )));
                    return;
                }
                Err(error) => {
                    dispatch.emit(fail(format!(
                        "part {} of {chunk_count} could not be sent: {error}",
                        index + 1
                    )));
                    return;
                }
            }

            index += 1;
        }

        // STEP THREE — assemble, make it servable, attach it. This is the slow one: the container decodes and
        // re-encodes the photograph, so a large file pauses here for a moment before the gallery refreshes.
        let complete = match web_sys::FormData::new() {
            Ok(form) => form,
            Err(_) => {
                dispatch.emit(fail("the browser could not create the upload form.".into()));
                return;
            }
        };
        if complete.append_with_str("step", "complete").is_err()
            || complete
                .append_with_str("propertyId", &property_id)
                .is_err()
            || complete.append_with_str("uploadId", &upload_id).is_err()
        {
            dispatch.emit(fail(
                "the upload could not be finished in the browser.".into(),
            ));
            return;
        }

        let request = match Request::post(OPS_MEDIA_CHUNKED_PATH).body(complete) {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(fail(format!("the upload could not be completed: {error}")));
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) if response.ok() => Msg::OpsMediaUploadCompleted {
                screen: screen.to_string(),
                generation,
            },
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: bridge_error_message(&body, status),
                }
            }
            Err(error) => fail(format!("the upload could not be completed: {error}")),
        };
        dispatch.emit(msg);
    });
}

fn run_flight_recorder_read(
    screen: &'static str,
    generation: u64,
    instance_id: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let url = format!("{FLIGHT_RECORDER_PATH}/{}", encode_component(&instance_id));
        let answer = Request::get(&url).send().await;
        let msg = match answer {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                if status >= 200 && status < 300 {
                    match serde_json::from_str::<serde_json::Value>(&body) {
                        Ok(transaction) => Msg::FlightRecorderLoaded {
                            screen: screen.to_string(),
                            generation,
                            instance_id,
                            transaction,
                        },
                        Err(error) => Msg::EffectFailed {
                            screen: screen.to_string(),
                            generation,
                            message: format!(
                                "the Flight Recorder answer could not be decoded: {error}"
                            ),
                        },
                    }
                } else {
                    let value = serde_json::from_str::<serde_json::Value>(&body).ok();
                    let message = value
                        .as_ref()
                        .and_then(|item| {
                            item.get("detail")
                                .or_else(|| item.get("error"))
                                .and_then(serde_json::Value::as_str)
                        })
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            if status == 404 {
                                "Trace not found for this process instance.".into()
                            } else {
                                format!("the Flight Recorder request failed with {status}")
                            }
                        });
                    Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message,
                    }
                }
            }
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the Flight Recorder request could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_tech_command(
    screen: &'static str,
    generation: u64,
    body: serde_json::Value,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let request = match Request::post(TECH_PATH)
            .header("content-type", "application/json")
            .body(body.to_string())
        {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the TECH command could not be built: {error}"),
                });
                return;
            }
        };

        let msg = match request.send().await {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                let value = serde_json::from_str::<serde_json::Value>(&text).ok();
                if status >= 200 && status < 300 {
                    let ok = value
                        .as_ref()
                        .and_then(|item| item.get("ok"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(true);
                    let message = value
                        .as_ref()
                        .and_then(|item| item.get("message"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("TECH command completed.")
                        .to_string();
                    Msg::TechCommandCompleted {
                        screen: screen.to_string(),
                        generation,
                        ok,
                        message,
                    }
                } else {
                    let message = value
                        .as_ref()
                        .and_then(|item| item.get("error").or_else(|| item.get("message")))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("the TECH command failed with {status}"));
                    Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message,
                    }
                }
            }
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the TECH command could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_cockpit_command(
    screen: &'static str,
    generation: u64,
    task_id: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let body = serde_json::json!({
            "action": "completeTask",
            "taskId": task_id,
        })
        .to_string();
        let request = match Request::post(COCKPIT_PATH)
            .header("content-type", "application/json")
            .body(body)
        {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the Cockpit command could not be built: {error}"),
                });
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => Msg::portal_loaded_json(screen, generation, &body),
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the Cockpit command answer could not be read: {error}"),
                },
            },
            Ok(response) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the Cockpit command failed with {}", response.status()),
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the Cockpit command could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_deal_people_search(
    screen: &'static str,
    generation: u64,
    query_value: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let url = format!(
            "{DEALS_PATH}?peopleSearch={}",
            encode_component(&query_value)
        );
        let answer = Request::get(&url).send().await;
        let msg = match answer {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => match serde_json::from_str::<PortalDealPeopleSearch>(&body) {
                    Ok(payload) => Msg::DealPeopleLoaded {
                        screen: screen.to_string(),
                        generation,
                        query: query_value,
                        people: payload.people,
                    },
                    Err(error) => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: format!("the client search answer could not be read: {error}"),
                    },
                },
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the client search answer could not be read: {error}"),
                },
            },
            Ok(response) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the client search failed with {}", response.status()),
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the client search could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_deal_create(
    screen: &'static str,
    generation: u64,
    property_id: String,
    client_person_id: String,
    owner_user_id: Option<String>,
    notes: Option<String>,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let body = serde_json::json!({
            "propertyId": property_id,
            "clientPersonId": client_person_id,
            "ownerUserId": owner_user_id,
            "notes": notes,
        })
        .to_string();
        let request = match Request::post(DEALS_PATH)
            .header("content-type", "application/json")
            .body(body)
        {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the contract create request could not be built: {error}"),
                });
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => match serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                    }) {
                    Some(id) => Msg::DealCreated {
                        screen: screen.to_string(),
                        generation,
                        id,
                    },
                    None => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: "the contract create answer did not contain id".into(),
                    },
                },
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the contract create answer could not be read: {error}"),
                },
            },
            Ok(response) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!(
                    "the contract create request failed with {}",
                    response.status()
                ),
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the contract create request could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_deal_workspace_people_search(
    screen: &'static str,
    generation: u64,
    purpose: String,
    query_value: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let url = format!(
            "{DEALS_PATH}?peopleSearch={}",
            encode_component(&query_value)
        );
        let answer = Request::get(&url).send().await;
        let msg = match answer {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => match serde_json::from_str::<PortalDealPeopleSearch>(&body) {
                    Ok(payload) => Msg::DealWorkspacePeopleLoaded {
                        screen: screen.to_string(),
                        generation,
                        purpose,
                        query: query_value,
                        people: payload.people,
                    },
                    Err(error) => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: format!("the workspace people answer could not be read: {error}"),
                    },
                },
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the workspace people answer could not be read: {error}"),
                },
            },
            Ok(response) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!(
                    "the workspace people search failed with {}",
                    response.status()
                ),
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the workspace people search could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_deal_workspace_command(
    screen: &'static str,
    generation: u64,
    deal_id: String,
    command: crate::model::PortalDealCommand,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let body = match serde_json::to_string(&command) {
            Ok(body) => body,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the workspace command could not be encoded: {error}"),
                });
                return;
            }
        };
        let url = format!("{DEALS_PATH}?scope={}", encode_component(&deal_id));
        let request = match Request::post(&url)
            .header("content-type", "application/json")
            .body(body)
        {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the workspace command could not be built: {error}"),
                });
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => match serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                    }) {
                    Some(id) => Msg::DealWorkspaceCommandCompleted {
                        screen: screen.to_string(),
                        generation,
                        id,
                    },
                    None => Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: "the workspace command answer did not contain id".into(),
                    },
                },
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the workspace command answer could not be read: {error}"),
                },
            },
            Ok(response) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the workspace command failed with {}", response.status()),
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the workspace command could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_projects_command(
    screen: &'static str,
    generation: u64,
    body: serde_json::Value,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let request = match Request::post(PROJECTS_PATH)
            .header("content-type", "application/json")
            .body(body.to_string())
        {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the Projects command could not be built: {error}"),
                });
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => Msg::portal_loaded_json(screen, generation, &body),
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the Projects command answer could not be read: {error}"),
                },
            },
            Ok(response) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the Projects command failed with {}", response.status()),
            },
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the Projects command could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

const FAVORITES_KEY: &str = "culebraluxe:saved-properties";
const RECENT_KEY: &str = "culebraluxe:recently-viewed";

const COMPARE_KEY: &str = "culebraluxe:compare-properties";
const SAVED_SEARCHES_KEY: &str = "culebraluxe:saved-searches";

/// A stored JSON array, keeping only the entries that parse as `T`.
fn parse_entries<T: serde::de::DeserializeOwned>(raw: &str) -> Vec<T> {
    serde_json::from_str::<Vec<serde_json::Value>>(raw)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| serde_json::from_value(entry).ok())
        .collect()
}

/// Persist a list to the device and tell any other listener, as the TypeScript stores did. A no-op write stays
/// silent. Storage can be unavailable (private browsing); then the reducer's copy is all there is for this visit.
fn write_store<T: serde::Serialize>(key: &str, entries: &[T], event: &str) {
    let Some(storage) = browser_storage() else {
        return;
    };
    let Ok(next) = serde_json::to_string(entries) else {
        return;
    };
    if storage.get_item(key).ok().flatten().as_deref() == Some(next.as_str()) {
        return;
    }
    if storage.set_item(key, &next).is_ok() {
        if let (Some(window), Ok(event)) = (web_sys::window(), web_sys::CustomEvent::new(event)) {
            let _ = window.dispatch_event(&event);
        }
    }
}

fn browser_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn favorite_entries(storage: &web_sys::Storage) -> Vec<serde_json::Value> {
    storage
        .get_item(FAVORITES_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok())
        .unwrap_or_default()
}

fn favorite_id(entry: &serde_json::Value) -> Option<&str> {
    entry
        .as_str()
        .or_else(|| entry.get("id").and_then(serde_json::Value::as_str))
}

fn read_property_browser(
    id: String,
    slug: String,
    title: String,
    valid_slugs: Vec<String>,
    dispatch: &Callback<Msg>,
) {
    let Some(storage) = browser_storage() else {
        dispatch.emit(Msg::PropertyBrowserLoaded {
            id,
            saved: false,
            recent: Vec::new(),
        });
        return;
    };
    let saved = favorite_entries(&storage)
        .iter()
        .any(|entry| favorite_id(entry) == Some(id.as_str()));
    let existing = storage
        .get_item(RECENT_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Vec<PropertyRecent>>(&raw).ok())
        .unwrap_or_default();
    let mut recorded = vec![PropertyRecent {
        slug: slug.clone(),
        id: id.clone(),
        name: title,
        at: js_sys::Date::now() as i64,
    }];
    recorded.extend(existing.into_iter().filter(|entry| entry.id != id));
    recorded.truncate(6);
    recorded.retain(|entry| entry.slug == slug || valid_slugs.contains(&entry.slug));
    if let Ok(serialized) = serde_json::to_string(&recorded) {
        let _ = storage.set_item(RECENT_KEY, &serialized);
    }
    let recent = recorded
        .into_iter()
        .filter(|entry| entry.slug != slug)
        .collect();
    dispatch.emit(Msg::PropertyBrowserLoaded { id, saved, recent });
}

fn write_property_favorite(
    id: String,
    slug: String,
    title: String,
    saved: bool,
    dispatch: &Callback<Msg>,
) {
    let Some(storage) = browser_storage() else {
        dispatch.emit(Msg::PropertyFavoriteStored { id, saved: false });
        return;
    };
    let mut entries = favorite_entries(&storage);
    entries.retain(|entry| favorite_id(entry) != Some(id.as_str()));
    if saved {
        entries.push(serde_json::json!({ "id": id.clone(), "slug": slug, "name": title }));
    }
    if let Ok(serialized) = serde_json::to_string(&entries) {
        if storage.set_item(FAVORITES_KEY, &serialized).is_ok() {
            if let Some(window) = web_sys::window() {
                if let Ok(event) = web_sys::CustomEvent::new("culebraluxe:favorites-changed") {
                    let _ = window.dispatch_event(&event);
                }
            }
        }
    }
    let actual = favorite_entries(&storage)
        .iter()
        .any(|entry| favorite_id(entry) == Some(id.as_str()));
    dispatch.emit(Msg::PropertyFavoriteStored { id, saved: actual });
}

const WEBSITE_INTAKE_PATH: &str = "/api/rust-ui/website-intake";

/// Post a contact form submission. Anything but an accepted answer is the failed state: the visitor keeps their typing
/// and is told, and the route has already recorded why.
fn submit_contact(
    submission: crate::model::ContactSubmission,
    submission_id: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    let request_type = if submission.request_type.is_empty() {
        "general_enquiry".to_string()
    } else {
        submission.request_type.clone()
    };
    let mut body = serde_json::json!({
        "submissionId": submission_id,
        "requestType": request_type,
        "name": submission.name,
        "email": submission.email,
        "message": submission.message,
        "company": submission.company,
    });
    if !submission.property_id.is_empty() {
        body["propertyId"] = serde_json::Value::String(submission.property_id.clone());
    }
    if !submission.service.is_empty() {
        body["service"] = serde_json::Value::String(submission.service.clone());
    }
    spawn_local(async move {
        let accepted = match Request::post(WEBSITE_INTAKE_PATH).json(&body) {
            Ok(request) => match request.send().await {
                Ok(response) if response.ok() => response
                    .json::<serde_json::Value>()
                    .await
                    .ok()
                    .and_then(|value| value.get("accepted").and_then(serde_json::Value::as_bool))
                    .unwrap_or(false),
                _ => false,
            },
            Err(_) => false,
        };
        dispatch.emit(Msg::ContactResult { accepted });
    });
}

fn fetch_entitlements(generation: u64, dispatch: &Callback<Msg>) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let grants = match Request::get("/api/portal/rust-ui/entitlements")
            .send()
            .await
        {
            Ok(response) if response.ok() => response.json::<PortalEntitlements>().await.ok(),
            _ => None,
        };
        dispatch.emit(match grants {
            Some(grants) => Msg::EntitlementsLoaded { generation, grants },
            None => Msg::EntitlementsUnavailable { generation },
        });
    });
}

#[derive(serde::Deserialize)]
struct RoleGrantResponse {
    roles: Vec<PortalRoleEntitlements>,
}

fn set_role_entitlement(
    generation: u64,
    role_code: String,
    action: String,
    granted: bool,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let payload =
            serde_json::json!({ "roleCode": role_code, "action": action, "granted": granted });
        let message = match Request::put(ROLE_ENTITLEMENTS_PATH)
            .header("content-type", "application/json")
            .body(payload.to_string())
        {
            Ok(request) => match request.send().await {
                Ok(response) if response.ok() => match response.json::<RoleGrantResponse>().await {
                    Ok(value) => Msg::SecurityRoleGrantChanged {
                        generation,
                        roles: value.roles,
                    },
                    Err(error) => Msg::EffectFailed {
                        screen: "security".into(),
                        generation,
                        message: format!("Role grant response could not be read: {error}"),
                    },
                },
                Ok(response) => {
                    let status = response.status();
                    let body = response.json::<serde_json::Value>().await.ok();
                    let detail = body
                        .as_ref()
                        .and_then(|value| value.get("error"))
                        .and_then(serde_json::Value::as_str);
                    Msg::EffectFailed {
                        screen: "security".into(),
                        generation,
                        message: detail
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("Role grant update failed ({status}).")),
                    }
                }
                Err(error) => Msg::EffectFailed {
                    screen: "security".into(),
                    generation,
                    message: format!("Role grant request failed: {error}"),
                },
            },
            Err(error) => Msg::EffectFailed {
                screen: "security".into(),
                generation,
                message: format!("Role grant request could not be built: {error}"),
            },
        };
        dispatch.emit(message);
    });
}

#[derive(serde::Deserialize)]
struct SecurityUsersResponse {
    users: Vec<PortalSecurityUser>,
}

fn set_user_primary_role(
    generation: u64,
    app_user_id: String,
    role_code: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let payload = serde_json::json!({ "appUserId": app_user_id, "roleCode": role_code });
        let message = match Request::put(SECURITY_USERS_PATH)
            .header("content-type", "application/json")
            .body(payload.to_string())
        {
            Ok(request) => match request.send().await {
                Ok(response) if response.ok() => {
                    match response.json::<SecurityUsersResponse>().await {
                        Ok(value) => Msg::SecurityUserRoleChanged {
                            generation,
                            users: value.users,
                        },
                        Err(error) => Msg::EffectFailed {
                            screen: "settings-users".into(),
                            generation,
                            message: format!("User role response could not be read: {error}"),
                        },
                    }
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.json::<serde_json::Value>().await.ok();
                    let detail = body
                        .as_ref()
                        .and_then(|value| value.get("error"))
                        .and_then(serde_json::Value::as_str);
                    Msg::EffectFailed {
                        screen: "settings-users".into(),
                        generation,
                        message: detail
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("User role update failed ({status}).")),
                    }
                }
                Err(error) => Msg::EffectFailed {
                    screen: "settings-users".into(),
                    generation,
                    message: format!("User role request failed: {error}"),
                },
            },
            Err(error) => Msg::EffectFailed {
                screen: "settings-users".into(),
                generation,
                message: format!("User role request could not be built: {error}"),
            },
        };
        dispatch.emit(message);
    });
}

fn run_read(effect: Effect, dispatch: &Callback<Msg>) {
    let (url, screen, generation, kind) = match effect {
        // A COMMAND IS NOT A READ and cannot arrive here: every command effect has its own runner, matched in `run`
        // before its catch-all passes anything unhandled to this function. The arm is here because the match must be
        // exhaustive — and a command falling through to a read would be a request to the wrong route with the wrong verb,
        // which is exactly the kind of silence this file exists to avoid.
        Effect::PropertyBrowserRead { .. }
        | Effect::PropertyFavoriteWrite { .. }
        | Effect::ListingFavoritesRead
        | Effect::BuyerToolsRead
        | Effect::CompareWrite(_)
        | Effect::SavedSearchesWrite(_)
        | Effect::SubmitContact { .. } => {
            unreachable!("Property browser effects are run before network reads")
        }
        Effect::SaveOps { .. } | Effect::CreateOpsProperty { .. } => {
            unreachable!("OPPS workbench commands are run by `run_ops_command`")
        }
        Effect::SearchOpsPeople { .. } => {
            unreachable!("OPPS Person search is run by `run_ops_people_search`")
        }
        Effect::UploadOpsMedia { .. } => {
            unreachable!("OPPS Media uploads are run by `run_ops_media_upload`")
        }
        Effect::AccountingCommand { .. } => {
            unreachable!("Accounting commands are run by `run_accounting_command`")
        }
        Effect::RecordArchive { .. } => {
            unreachable!("Records commands are run by `run_record_archive`")
        }
        Effect::UploadListingMedia { .. } => {
            unreachable!("Listing Media uploads are run by `run_listing_media_upload`")
        }
        Effect::FetchFlightRecorder { .. } => {
            unreachable!("Flight Recorder reads are run by `run_flight_recorder_read`")
        }
        Effect::TechCommand { .. } => {
            unreachable!("TECH commands are run by `run_tech_command`")
        }
        Effect::FetchTech {
            screen,
            selected,
            generation,
        } => (
            tech_query(selected.as_deref()),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchCabinet { screen, generation } => {
            (CABINET_PATH.to_string(), screen, generation, Kind::Portal)
        }
        Effect::FetchCockpit { screen, generation } => {
            (COCKPIT_PATH.to_string(), screen, generation, Kind::Portal)
        }
        Effect::FetchPage {
            screen,
            scope,
            generation,
        } => (
            query(PAGE_PATH, screen, scope.as_deref()),
            screen,
            generation,
            Kind::Page,
        ),
        Effect::FetchAccountingPnl {
            screen,
            generation,
            from,
            to,
        } => (
            pnl_query(screen, &from, &to),
            screen,
            generation,
            Kind::Portal,
        ),

        Effect::FetchOps {
            screen,
            entity,
            selected,
            search,
            page,
            generation,
        } => (
            ops_workbench_query(&entity, selected.as_deref(), &search, page),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchRecords {
            screen,
            selected,
            search,
            page,
            generation,
        } => (
            ops_query(RECORDS_PATH, selected.as_deref(), &search, page),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchListingMedia {
            screen,
            selected,
            search,
            page,
            generation,
        } => (
            ops_query(LISTING_MEDIA_PATH, selected.as_deref(), &search, page),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchPortal {
            screen,
            scope,
            generation,
        } => (
            query(PORTAL_PATH, screen, scope.as_deref()),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchClients {
            screen,
            scope,
            selected,
            search,
            page,
            generation,
        } => (
            clients_query(screen, scope.as_deref(), selected.as_deref(), &search, page),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchForms {
            screen,
            scope,
            generation,
        } => (
            query(FORMS_PATH, screen, scope.as_deref()),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchProjects { screen, generation } => {
            (PROJECTS_PATH.to_string(), screen, generation, Kind::Portal)
        }
        Effect::FetchDeals {
            screen,
            scope,
            generation,
        } => (
            query(DEALS_PATH, screen, scope.as_deref()),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchRows {
            screen,
            scope,
            generation,
        } => (
            query(ROWS_PATH, screen, scope.as_deref()),
            screen,
            generation,
            Kind::Rows,
        ),
        Effect::CompleteCockpitTask { .. }
        | Effect::SaveForm { .. }
        | Effect::CreateForm { .. }
        | Effect::SearchDealPeople { .. }
        | Effect::CreateDeal { .. }
        | Effect::SearchDealWorkspacePeople { .. }
        | Effect::RunDealWorkspaceCommand { .. }
        | Effect::UpdateProjectStatus { .. }
        | Effect::SaveProjectWork { .. }
        | Effect::BrowserNavigate { .. }
        | Effect::FetchEntitlements { .. }
        | Effect::SetRoleEntitlement { .. }
        | Effect::SetUserPrimaryRole { .. } => return,
    };

    let dispatch = dispatch.clone();
    spawn_local(async move {
        let answer = Request::get(&url).send().await;
        let fail = |message: String| Msg::EffectFailed {
            screen: screen.to_string(),
            generation,
            message,
        };
        let msg = match answer {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => match kind {
                    Kind::Page => Msg::page_loaded_json(screen, generation, &body),
                    Kind::Portal => Msg::portal_loaded_json(screen, generation, &body),
                    Kind::Rows => Msg::rows_loaded_json(screen, generation, &body),
                },
                Err(error) => fail(format!("the answer could not be read: {error}")),
            },
            Ok(response) => fail(format!(
                "the {} request failed with {}",
                match kind {
                    Kind::Page => "page",
                    Kind::Portal => "portal",
                    Kind::Rows => "rows",
                },
                response.status()
            )),
            Err(error) => fail(format!("the request could not be sent: {error}")),
        };
        dispatch.emit(msg);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Page,
    Portal,
    Rows,
}

/// The P&L's request: the screen, and the period it is asking about.
///
/// THE RANGE IS ALWAYS ON THE REQUEST, even when both ends are empty. An empty pair means "the period the screen has not
/// chosen yet", which the bridge answers with the current month — so "nothing chosen" and "the current month" stay one thing
/// rather than two.
fn tech_query(selected: Option<&str>) -> String {
    match selected.filter(|value| !value.is_empty()) {
        Some(id) => format!("{TECH_PATH}?selected={}", encode_component(id)),
        None => TECH_PATH.to_string(),
    }
}

fn pnl_query(screen: &str, from: &str, to: &str) -> String {
    format!(
        "{PORTAL_PATH}?screen={}&from={}&to={}",
        encode_component(screen),
        encode_component(from),
        encode_component(to)
    )
}

fn query(path: &str, screen: &str, scope: Option<&str>) -> String {
    match scope {
        Some(key) if !key.is_empty() => format!(
            "{path}?screen={}&scope={}",
            encode_component(screen),
            encode_component(key)
        ),
        _ => format!("{path}?screen={}", encode_component(screen)),
    }
}

fn ops_workbench_query(entity: &str, selected: Option<&str>, search: &str, page: usize) -> String {
    let mut url = format!(
        "{OPPS_PATH}?entity={}&page={}&search={}",
        encode_component(entity),
        page,
        encode_component(search)
    );
    if let Some(selected) = selected.filter(|value| !value.is_empty()) {
        url.push_str("&selected=");
        url.push_str(&encode_component(selected));
    }
    url
}

fn ops_query(path: &str, selected: Option<&str>, search: &str, page: usize) -> String {
    let mut url = format!("{path}?page={}&search={}", page, encode_component(search));
    if let Some(selected) = selected.filter(|value| !value.is_empty()) {
        url.push_str("&selected=");
        url.push_str(&encode_component(selected));
    }
    url
}

fn clients_query(
    screen: &str,
    scope: Option<&str>,
    selected: Option<&str>,
    search: &str,
    page: usize,
) -> String {
    let mut url = format!(
        "{CLIENTS_PATH}?screen={}&page={}&search={}",
        encode_component(screen),
        page,
        encode_component(search)
    );
    if let Some(scope) = scope.filter(|value| !value.is_empty()) {
        url.push_str("&scope=");
        url.push_str(&encode_component(scope));
    }
    if let Some(selected) = selected.filter(|value| !value.is_empty()) {
        url.push_str("&selected=");
        url.push_str(&encode_component(selected));
    }
    url
}

fn encode_component(value: &str) -> String {
    use std::fmt::Write;

    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            let _ = write!(&mut encoded, "%{byte:02X}");
        }
    }
    encoded
}

/// Run one Accounting command.
///
/// THE ANSWER IS THE REFRESHED SCREEN, so a success is parsed exactly like a read: the payload becomes a `PortalLoaded`
/// for the screen that asked. On failure the bridge's own message is surfaced — "Vendor is required.", or the conflict for
/// a voided receivable — because a form that says only "failed" makes the operator guess which of their fields was wrong.
fn run_record_archive(
    screen: &'static str,
    generation: u64,
    property_id: String,
    archived: bool,
    search: String,
    page: usize,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let action = if archived { "restore" } else { "archive" };
        let url = ops_query(RECORDS_PATH, Some(&property_id), &search, page);
        let request = match Request::post(&url)
            .header("content-type", "application/json")
            .body(
                serde_json::json!({
                    "action": action,
                    "propertyId": property_id,
                })
                .to_string(),
            ) {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the Records command could not be built: {error}"),
                });
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) if response.ok() => match response.text().await {
                Ok(body) => Msg::portal_loaded_json(screen, generation, &body),
                Err(error) => Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the Records answer could not be read: {error}"),
                },
            },
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: bridge_error_message(&body, status),
                }
            }
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the Records command could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

fn run_listing_media_upload(
    screen: &'static str,
    generation: u64,
    property_id: String,
    role: String,
    alt: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let fail = |message: String| Msg::EffectFailed {
            screen: screen.to_string(),
            generation,
            message,
        };

        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            dispatch.emit(fail("the browser document is unavailable.".into()));
            return;
        };
        let Some(element) = document.get_element_by_id("listing-media-file") else {
            dispatch.emit(fail("the Listing Media file input is unavailable.".into()));
            return;
        };
        let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>() else {
            dispatch.emit(fail(
                "the Listing Media file input has the wrong element type.".into(),
            ));
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else {
            dispatch.emit(fail("Choose an image before uploading.".into()));
            return;
        };

        let form = match web_sys::FormData::new() {
            Ok(form) => form,
            Err(_) => {
                dispatch.emit(fail("the browser could not create the upload form.".into()));
                return;
            }
        };
        if form.append_with_str("propertyId", &property_id).is_err()
            || form.append_with_str("role", &role).is_err()
            || (!alt.trim().is_empty() && form.append_with_str("altText", alt.trim()).is_err())
            || form
                .append_with_blob_and_filename(
                    "file",
                    file.unchecked_ref::<web_sys::Blob>(),
                    &file.name(),
                )
                .is_err()
        {
            dispatch.emit(fail(
                "the browser could not prepare the selected image.".into(),
            ));
            return;
        }

        let request = match Request::post(LISTING_MEDIA_UPLOAD_PATH).body(form) {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(fail(format!(
                    "the Listing Media upload could not be built: {error}"
                )));
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) if response.ok() => Msg::ListingMediaUploadCompleted {
                screen: screen.to_string(),
                generation,
            },
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: bridge_error_message(&body, status),
                }
            }
            Err(error) => fail(format!(
                "the Listing Media upload could not be sent: {error}"
            )),
        };
        dispatch.emit(msg);
    });
}

fn run_accounting_command(
    screen: &'static str,
    generation: u64,
    body: serde_json::Value,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let request = match Request::post(ACCOUNTING_PATH)
            .header("content-type", "application/json")
            .body(body.to_string())
        {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(Msg::EffectFailed {
                    screen: screen.to_string(),
                    generation,
                    message: format!("the Accounting command could not be built: {error}"),
                });
                return;
            }
        };
        let msg = match request.send().await {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                if status >= 200 && status < 300 {
                    Msg::portal_loaded_json(screen, generation, &text)
                } else {
                    Msg::EffectFailed {
                        screen: screen.to_string(),
                        generation,
                        message: bridge_error_message(&text, status),
                    }
                }
            }
            Err(error) => Msg::EffectFailed {
                screen: screen.to_string(),
                generation,
                message: format!("the Accounting command could not be sent: {error}"),
            },
        };
        dispatch.emit(msg);
    });
}

/// The message out of a failed bridge response: the `error` the route wrote, or the status when there is nothing to read.
///
/// A non-JSON body is not an error worth inventing words for — the status is still true, and a made-up sentence would be
/// less useful than the number.
fn bridge_error_message(body: &str, status: u16) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.as_str())
                .map(str::to_owned)
        })
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(|| format!("the Accounting command failed with {status}"))
}
