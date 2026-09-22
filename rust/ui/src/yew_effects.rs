//! The effects, run by Yew.
//!
//! The reducer decides what must happen; this module only performs those effects and returns completion messages.

use gloo_net::http::Request;
use yew::platform::spawn_local;
use yew::Callback;

use crate::model::{Effect, Msg};

const PAGE_PATH: &str = "/api/rust-ui/public-page";
const ROWS_PATH: &str = "/api/rust-ui/public-rows";
const PORTAL_PATH: &str = "/api/portal/rust-ui/page";
const COCKPIT_PATH: &str = "/api/portal/rust-ui/cockpit";
const CLIENTS_PATH: &str = "/api/portal/rust-ui/clients";
const FORMS_PATH: &str = "/api/portal/rust-ui/forms";
const PROJECTS_PATH: &str = "/api/portal/rust-ui/projects";

pub fn run(effect: Effect, dispatch: &Callback<Msg>) {
    match effect {
        Effect::CompleteCockpitTask {
            screen,
            generation,
            task_id,
        } => {
            run_cockpit_command(screen, generation, task_id, dispatch);
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
        | Effect::UpdateProjectStatus { .. }
        | Effect::SaveProjectWork { .. }
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
