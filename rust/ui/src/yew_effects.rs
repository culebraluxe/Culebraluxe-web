//! The effects, run by Yew.
//!
//! The reducer decides what must happen; this module only performs those effects and returns completion messages.

use gloo_net::http::Request;
use wasm_bindgen::JsCast;
use yew::platform::spawn_local;
use yew::Callback;

use crate::model::{Effect, Msg, PortalDealPeopleSearch};

const PAGE_PATH: &str = "/api/rust-ui/public-page";
const ROWS_PATH: &str = "/api/rust-ui/public-rows";
const PORTAL_PATH: &str = "/api/portal/rust-ui/page";
const TECH_PATH: &str = "/api/portal/rust-ui/tech";
const FLIGHT_RECORDER_PATH: &str = "/api/portal/flight-recorder";
const COCKPIT_PATH: &str = "/api/portal/rust-ui/cockpit";
const CABINET_PATH: &str = "/api/portal/rust-ui/cabinet";
const CLIENTS_PATH: &str = "/api/portal/rust-ui/clients";
const FORMS_PATH: &str = "/api/portal/rust-ui/forms";
const PROJECTS_PATH: &str = "/api/portal/rust-ui/projects";
const DEALS_PATH: &str = "/api/portal/rust-ui/deals";
const ACCOUNTING_PATH: &str = "/api/portal/rust-ui/accounting";
const RECORDS_PATH: &str = "/api/portal/rust-ui/records";
const LISTING_MEDIA_PATH: &str = "/api/portal/rust-ui/listing-media";
const LISTING_MEDIA_UPLOAD_PATH: &str = "/api/property-media/upload";

pub fn run(effect: Effect, dispatch: &Callback<Msg>) {
    match effect {
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
            run_listing_media_upload(
                screen,
                generation,
                property_id,
                role,
                alt,
                dispatch,
            );
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
            run_deal_workspace_people_search(
                screen,
                generation,
                purpose,
                query,
                dispatch,
            );
        }
        Effect::RunDealWorkspaceCommand {
            screen,
            generation,
            deal_id,
            command,
        } => {
            run_deal_workspace_command(
                screen,
                generation,
                deal_id,
                command,
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

fn run_flight_recorder_read(
    screen: &'static str,
    generation: u64,
    instance_id: String,
    dispatch: &Callback<Msg>,
) {
    let dispatch = dispatch.clone();
    spawn_local(async move {
        let url = format!(
            "{FLIGHT_RECORDER_PATH}/{}",
            encode_component(&instance_id)
        );
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
                    .and_then(|value| value.get("id").and_then(serde_json::Value::as_str).map(str::to_owned))
                {
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
                message: format!("the contract create request failed with {}", response.status()),
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
                message: format!("the workspace people search failed with {}", response.status()),
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
                    .and_then(|value| value.get("id").and_then(serde_json::Value::as_str).map(str::to_owned))
                {
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

fn run_read(effect: Effect, dispatch: &Callback<Msg>) {
    let (url, screen, generation, kind) = match effect {
        // A COMMAND IS NOT A READ and cannot arrive here: every command effect has its own runner, matched in `run`
        // before its catch-all passes anything unhandled to this function. The arm is here because the match must be
        // exhaustive — and a command falling through to a read would be a request to the wrong route with the wrong verb,
        // which is exactly the kind of silence this file exists to avoid.
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
        Effect::FetchCabinet { screen, generation } => (
            CABINET_PATH.to_string(),
            screen,
            generation,
            Kind::Portal,
        ),
        Effect::FetchCockpit { screen, generation } => (
            COCKPIT_PATH.to_string(),
            screen,
            generation,
            Kind::Portal,
        ),
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
        Effect::FetchProjects { screen, generation } => (
            PROJECTS_PATH.to_string(),
            screen,
            generation,
            Kind::Portal,
        ),
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
        | Effect::RecordArchive { .. }
        | Effect::UploadListingMedia { .. }
        | Effect::BrowserNavigate { .. } => return,
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

fn ops_query(
    path: &str,
    selected: Option<&str>,
    search: &str,
    page: usize,
) -> String {
    let mut url = format!(
        "{path}?page={}&search={}",
        page,
        encode_component(search)
    );
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
            )
        {
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
            dispatch.emit(fail("the Listing Media file input has the wrong element type.".into()));
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
            dispatch.emit(fail("the browser could not prepare the selected image.".into()));
            return;
        }

        let request = match Request::post(LISTING_MEDIA_UPLOAD_PATH).body(form) {
            Ok(request) => request,
            Err(error) => {
                dispatch.emit(fail(format!("the Listing Media upload could not be built: {error}")));
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
            Err(error) => fail(format!("the Listing Media upload could not be sent: {error}")),
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

