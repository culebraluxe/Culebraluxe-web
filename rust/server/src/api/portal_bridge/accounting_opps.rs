//! Accounting reads and actions, the OPPS Data Workbench, and listing media.

#[allow(unused_imports)]
use super::*;

pub(super) const ACCOUNTING_SCREENS: &[&str] =
    &["accounting", "accounting-expenses", "accounting-receivables", "accounting-pnl", "accounting-receipt-scanner"];

/// The book's date (UTC, as the relay's `todayISO`), which the create forms default to.
pub(super) fn book_today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}

/// One Accounting screen's payload, `{ accounting: ... }`. The P&L period defaults to the current month.
pub(super) async fn accounting_payload(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    screen: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Value, ApiError> {
    use chrono::Datelike;
    let accounting = state.services().accounting();
    let context = &resolved.service;
    let today = book_today();
    let body = match screen {
        "accounting" => {
            let (dashboard, forecast) = tokio::join!(
                accounting.dashboard(context),
                accounting.commission_forecast(context),
            );
            json!({
                "dashboard": to_json(dashboard.map_err(failed(resolved))?),
                "commissionForecast": to_json(forecast.map_err(failed(resolved))?),
            })
        },
        "accounting-expenses" => {
            let (expenses, categories) = tokio::join!(accounting.expenses(context), accounting.expense_categories(context));
            json!({
                "expenses": to_json(expenses.map_err(failed(resolved))?),
                "expenseCategories": to_json(categories.map_err(failed(resolved))?),
                "today": today.to_string(),
            })
        }
        "accounting-receivables" => json!({
            "receivables": to_json(accounting.receivables(context).await.map_err(failed(resolved))?),
            "today": today.to_string(),
        }),
        "accounting-pnl" => {
            let first = today.with_day(1).unwrap_or(today);
            let last = first
                .checked_add_months(chrono::Months::new(1))
                .and_then(|next| next.pred_opt())
                .unwrap_or(today);
            let pick = |value: Option<&str>, fallback: chrono::NaiveDate| {
                value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).unwrap_or_else(|| fallback.to_string())
            };
            let request = domain::PnlRequest { from: pick(from, first), to: pick(to, last) };
            json!({ "pnl": to_json(accounting.pnl(&request, context).await.map_err(failed(resolved))?) })
        }
        _ => json!({ "today": today.to_string() }),
    };
    Ok(json!({ "accounting": body }))
}

/// Accounting's three commands; each answers the screen that asked, as it now stands.
pub(super) async fn accounting_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let raw = |key: &str| body.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
    let text = |key: &str| body.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
    let screen = raw("screen");
    if !ACCOUNTING_SCREENS.contains(&screen.as_str()) {
        return Err(ApiError::bad_request("ACCOUNTING_SCREEN_UNKNOWN", format!("no accounting screen '{screen}'")));
    }
    let accounting = state.services().accounting();
    let context = &resolved.service;
    match raw("action").as_str() {
        "createExpense" => {
            let command = domain::CreateExpenseCommand {
                vendor: raw("vendor"),
                category: raw("category"),
                // The digits the operator typed: Rust validates the decimal, nothing rounds it first.
                amount: raw("amount"),
                expense_on: raw("expenseOn"),
                memo: text("memo"),
                deal_id: text("dealId"),
                property_id: text("propertyId"),
                person_id: text("personId"),
            };
            accounting.create_expense(&command, context).await.map_err(failed(&resolved))?;
        }
        "createReceivable" => {
            let command = domain::CreateReceivableCommand {
                reference: text("reference"),
                description: raw("description"),
                category: raw("category"),
                amount: raw("amount"),
                issued_on: raw("issuedOn"),
                due_on: text("dueOn"),
                deal_id: text("dealId"),
                property_id: text("propertyId"),
                person_id: text("personId"),
            };
            accounting.create_receivable(&command, context).await.map_err(failed(&resolved))?;
        }
        "markReceivablePaid" => {
            let Some(receivable_id) = text("receivableId") else {
                return Err(ApiError::bad_request("RECEIVABLE_REQUIRED", "A receivable is required."));
            };
            let command = domain::MarkReceivablePaidCommand { receivable_id, paid_on: raw("paidOn") };
            accounting.mark_receivable_paid(&command, context).await.map_err(failed(&resolved))?;
        }
        other => {
            return Err(ApiError::bad_request("ACCOUNTING_COMMAND_UNKNOWN", format!("no accounting command '{other}'")))
        }
    }
    let from = text("from");
    let to = text("to");
    Ok(Json(accounting_payload(&state, &resolved, &screen, from.as_deref(), to.as_deref()).await?))
}

pub(super) const OPPS_PAGE_SIZE: i64 = 50;

#[derive(Debug, Deserialize)]
pub(super) struct OppsQuery {
    pub(super) entity: Option<String>,
    #[serde(default)]
    pub(super) search: String,
    pub(super) page: Option<String>,
    pub(super) selected: Option<String>,
}

pub(super) fn opps_entity(value: Option<&str>) -> &'static str {
    match value {
        Some("person") => "person",
        Some("project") => "project",
        _ => "property",
    }
}

pub(super) fn opps_row(id: &Value, title: &Value, subtitle: &Value, status: &Value, meta: &Value) -> Value {
    json!({ "id": id, "title": title, "subtitle": subtitle, "status": status, "meta": meta })
}

pub(super) fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&Value::Null)
}

/// The Data Workbench: one entity's list (property, person or project), a page of it, and the selected record.
pub(super) async fn opps_workbench(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    entity: &str,
    search: &str,
    page_index: i64,
    selected: Option<String>,
) -> Result<Value, ApiError> {
    let services = state.services();
    let context = &resolved.service;
    let empty = Vec::new();
    let mut payload = json!({
        "entity": entity, "rows": [], "total": 0, "page": page_index + 1, "pageSize": OPPS_PAGE_SIZE,
        "selectedId": null, "property": null, "person": null, "project": null, "media": [],
    });
    match entity {
        "person" => {
            let request = domain::ClientAdminPageRequest {
                search: search.to_owned(),
                page: page_index + 1,
                page_size: OPPS_PAGE_SIZE,
            };
            let page = to_json(services.clients().admin(&request, context).await.map_err(failed(resolved))?);
            let rows = page.get("rows").and_then(Value::as_array).unwrap_or(&empty);
            let selected_id = selected
                .filter(|id| rows.iter().any(|row| str_at(row, "id") == Some(id.as_str())))
                .or_else(|| rows.first().and_then(|row| str_at(row, "id")).map(str::to_owned));
            if let Some(id) = &selected_id {
                let (people, clients) = (services.person(), services.clients());
                let (canonical, client) = tokio::join!(people.get(id, context), clients.detail(id, context));
                let canonical = to_json(canonical.map_err(failed(resolved))?);
                let client = to_json(client.map_err(failed(resolved))?);
                payload["person"] = json!({
                    "id": at(&canonical, "id"),
                    "displayName": at(&canonical, "display_name"),
                    "role": client.get("role").filter(|v| !v.is_null()).cloned().unwrap_or(json!("unclassified")),
                    "status": at(&canonical, "status"),
                    "company": at(&canonical, "company"),
                    "civilStatus": at(&canonical, "civil_status"),
                    "location": at(&client, "location"),
                    "email": at(&client, "email"),
                    "phone": at(&client, "phone"),
                });
            }
            payload["rows"] = rows
                .iter()
                .map(|row| {
                    let meta = row.get("primaryEmail").filter(|v| !v.is_null()).unwrap_or(at(row, "primaryPhone"));
                    opps_row(at(row, "id"), at(row, "displayName"), at(row, "location"), at(row, "status"), meta)
                })
                .collect();
            payload["total"] = at(&page, "total").clone();
            payload["page"] = at(&page, "page").clone();
            payload["pageSize"] = at(&page, "pageSize").clone();
            payload["selectedId"] = json!(selected_id);
        }
        "project" => {
            let all = services.project().lock().await.list(context).await;
            let all = to_json(all.map_err(|error| correlate(ApiError::from(error), resolved))?);
            let all = all.as_array().unwrap_or(&empty);
            let needle = search.trim().to_lowercase();
            let filtered: Vec<&Value> = all
                .iter()
                .filter(|project| {
                    if needle.is_empty() {
                        return true;
                    }
                    let mut text: Vec<String> = ["name", "owner", "status", "description", "project_type"]
                        .iter()
                        .filter_map(|key| str_at(project, key).map(str::to_owned))
                        .collect();
                    text.extend(at(project, "areas").as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_owned)));
                    text.join(" ").to_lowercase().contains(&needle)
                })
                .collect();
            let start = (page_index * OPPS_PAGE_SIZE) as usize;
            let rows: Vec<&Value> = filtered.iter().skip(start).take(OPPS_PAGE_SIZE as usize).copied().collect();
            let selected_id = selected
                .filter(|id| rows.iter().any(|row| str_at(row, "id") == Some(id.as_str())))
                .or_else(|| rows.first().and_then(|row| str_at(row, "id")).map(str::to_owned));
            if let Some(project) = selected_id.as_deref().and_then(|id| all.iter().find(|p| str_at(p, "id") == Some(id))) {
                let mut project = camel_keys(project.clone());
                if let Some(object) = project.as_object_mut() {
                    object.remove("createdAt");
                    object.remove("updatedAt");
                }
                payload["project"] = project;
            }
            payload["rows"] = rows
                .iter()
                .map(|row| opps_row(at(row, "id"), at(row, "name"), at(row, "project_type"), at(row, "status"), at(row, "owner")))
                .collect();
            payload["total"] = json!(filtered.len());
            payload["selectedId"] = json!(selected_id);
        }
        _ => {
            let property = services.property();
            let request = domain::PropertyAdminPageRequest {
                search: search.to_owned(),
                page: page_index + 1,
                page_size: OPPS_PAGE_SIZE,
            };
            let page = to_json(property.admin_page(&request, context).await.map_err(failed(resolved))?);
            let rows = page.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
            let selected_id = selected.or_else(|| rows.first().and_then(|row| str_at(row, "id")).map(str::to_owned));
            let status_of = |row: &Value| {
                if at(row, "archived").as_bool() == Some(true) { json!("archived") } else { at(row, "status").clone() }
            };
            let mut out: Vec<Value> = rows
                .iter()
                .map(|row| opps_row(at(row, "id"), at(row, "name"), at(row, "location"), &status_of(row), at(row, "listPrice")))
                .collect();
            if let Some(id) = &selected_id {
                let media_service = services.media();
                let (detail, media) = tokio::join!(property.admin_get(id, context), media_service.for_property(id, context));
                let detail = detail.map_err(failed(resolved))?.ok_or_else(|| {
                    correlate(ApiError::not_found("PROPERTY_NOT_FOUND", format!("Property not found: {id}")), resolved)
                })?;
                let mut detail = to_json(detail);
                let seller = match str_at(&detail, "sellerPersonId").map(str::to_owned) {
                    Some(seller_id) => to_json(services.clients().detail(&seller_id, context).await.map_err(failed(resolved))?),
                    None => Value::Null,
                };
                if let Some(object) = detail.as_object_mut() {
                    if let Some(name) = seller.get("displayName").filter(|v| !v.is_null()) {
                        object.insert("sellerName".into(), name.clone());
                    }
                    object.insert("sellerEmail".into(), at(&seller, "email").clone());
                    object.insert("sellerPhone".into(), at(&seller, "phone").clone());
                    object.insert("sellerLocation".into(), at(&seller, "location").clone());
                }
                if !out.iter().any(|row| str_at(row, "id") == Some(id.as_str())) {
                    out.insert(0, opps_row(at(&detail, "id"), at(&detail, "name"), at(&detail, "location"), &status_of(&detail), at(&detail, "listPrice")));
                }
                payload["property"] = detail;
                payload["media"] = to_json(media.map_err(failed(resolved))?);
            }
            payload["rows"] = Value::Array(out);
            payload["total"] = at(&page, "total").clone();
            payload["page"] = at(&page, "page").clone();
            payload["pageSize"] = at(&page, "pageSize").clone();
            payload["selectedId"] = json!(selected_id);
        }
    }
    Ok(json!({ "ops": payload }))
}

pub(super) async fn opps(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OppsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let page_index = query.page.as_deref().and_then(|page| page.trim().parse::<i64>().ok()).unwrap_or(0).max(0);
    let selected = query.selected.map(|id| id.trim().to_owned()).filter(|id| !id.is_empty());
    let entity = opps_entity(query.entity.as_deref());
    Ok(Json(opps_workbench(&state, &resolved, entity, query.search.trim(), page_index, selected).await?))
}

/// The property save body, from the workbench's string fields: blanks are absent, flags are "true".
pub(super) fn property_save_body(fields: &serde_json::Map<String, Value>) -> Value {
    const FLAGS: &[&str] = &[
        "featured", "isActiveListing", "isPublished", "hasOceanView", "hasBayView", "hasBeachView", "hasHarborView",
        "hasIslandView", "hasMountainView", "hasSunriseView", "hasSunsetView", "hasWaterAccess", "hasBeachAccess",
        "hasPool", "hasGenerator", "hasSolar", "isFurnished", "isGated", "archived",
    ];
    const TEXT: &[&str] = &[
        "slug", "propertyType", "listPrice", "originalListPrice", "location", "addressLine1", "streetNumber", "streetName",
        "unitNumber", "city", "stateOrProvince", "neighborhood", "postalCode", "country", "isoCountryCode", "latitude",
        "longitude", "bedrooms", "bathrooms", "bathroomsFull", "bathroomsHalf", "squareFeet", "lotSize", "lotSizeUnits",
        "lotSizeAcres", "lotSizeSqft", "roadFrontageFeet", "roadSurfaceType", "lotDescription", "utilitiesNotes",
        "catastroNumber", "buildability", "slopeDescription", "poolPotential", "roadAdjacency", "utilitiesAvailability",
        "hoaStatus", "viewDescription", "yearBuilt", "stories", "parkingSpaces", "shortDescription",
        "editorialDescription", "publicRemarks", "seoTitle", "seoDescription", "heroTitle", "tagline",
        "architectureNotes", "amenitiesNotes", "lifestyleNotes", "listingAgentName", "listingAgentEmail",
        "listingAgentPhone", "listingOffice", "legalOwnerName", "listingIdentifier", "registryEntry", "fincaNumber",
        "registrySection", "sellerPersonId",
    ];
    const STELLAR: &[&str] = &[
        "listingContractDate", "expirationDate", "listingType", "agentMlsId", "taxId", "taxYear", "annualTax",
        "legalDescription", "zoning", "totalAreaSqft", "heatedAreaSource", "ownershipType", "hoaDetails",
        "showingInstructions", "occupantType",
    ];
    let raw = |key: &str| fields.get(key).and_then(Value::as_str).unwrap_or("");
    let clean = |key: &str| {
        let value = raw(key).trim();
        if value.is_empty() { Value::Null } else { json!(value) }
    };
    let mut body = serde_json::Map::new();
    body.insert("name".into(), json!(raw("name")));
    body.insert("status".into(), json!(if raw("status").is_empty() { "prospect" } else { raw("status") }));
    for key in FLAGS {
        body.insert((*key).into(), json!(raw(key) == "true"));
    }
    for key in TEXT {
        body.insert((*key).into(), clean(key));
    }
    let stellar: serde_json::Map<String, Value> = STELLAR.iter().map(|key| ((*key).to_owned(), clean(key))).collect();
    body.insert("stellar".into(), Value::Object(stellar));
    Value::Object(body)
}

/// The Workbench's writes: create a property, or save the open property, person or project; each answers the
/// workbench with the saved record selected.
pub(super) async fn opps_act(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(command): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let invalid = |error: serde_json::Error| ApiError::bad_request("WORKBENCH_SAVE_INVALID", error.to_string());
    if str_at(&command, "action") == Some("createProperty") {
        let body: CreatePropertyAdminBody = serde_json::from_value(json!({
            "name": at(&command, "name"),
            "propertyType": at(&command, "propertyType"),
        }))
        .map_err(invalid)?;
        let property = to_json(apply_property_admin_create(&state, &resolved, body).await?);
        let mut record = property.clone();
        if let Some(object) = record.as_object_mut() {
            for key in ["sellerEmail", "sellerPhone", "sellerLocation"] {
                object.insert(key.into(), Value::Null);
            }
        }
        return Ok(Json(json!({ "ops": {
            "entity": "property",
            "rows": [opps_row(at(&property, "id"), at(&property, "name"), at(&property, "location"), at(&property, "status"), at(&property, "listPrice"))],
            "total": 1, "page": 1, "pageSize": OPPS_PAGE_SIZE, "selectedId": at(&property, "id"),
            "property": record, "person": null, "project": null, "media": [],
        } })));
    }
    let id = str_at(&command, "id").map(str::trim).filter(|id| !id.is_empty()).map(str::to_owned);
    let (Some("save"), Some(id)) = (str_at(&command, "action"), id) else {
        return Err(ApiError::bad_request("WORKBENCH_SAVE_INVALID", "A workbench save requires an entity and id."));
    };
    let empty = serde_json::Map::new();
    let fields = command.get("fields").and_then(Value::as_object).unwrap_or(&empty);
    let field = |key: &str| fields.get(key).and_then(Value::as_str).unwrap_or("");
    let clean = |key: &str| {
        let value = field(key).trim();
        if value.is_empty() { Value::Null } else { json!(value) }
    };
    let entity = opps_entity(str_at(&command, "entity"));
    match entity {
        "property" => {
            let body: SavePropertyAdminBody = serde_json::from_value(property_save_body(fields)).map_err(invalid)?;
            apply_property_admin_save(&state, &resolved, id.clone(), body).await?;
        }
        "person" => {
            let body: UpdatePersonAdminBody = serde_json::from_value(json!({
                "displayName": field("displayName"),
                "status": field("status"),
                "company": clean("company"),
                "civilStatus": clean("civilStatus"),
                "location": field("location"),
                "email": field("email"),
                "phone": field("phone"),
            }))
            .map_err(invalid)?;
            apply_person_admin_update(&state, &resolved, id.clone(), body).await?;
        }
        _ => {
            let areas: Vec<String> =
                field("areas").split(',').map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned).collect();
            let version = field("playbookVersion").trim().parse::<i64>().ok();
            let body: UpdateProjectBody = serde_json::from_value(json!({
                "name": clean("name"), "owner": clean("owner"), "status": clean("status"),
                "description": field("description"), "areas": areas, "projectType": clean("projectType"),
                "playbookId": clean("playbookId"), "playbookVersion": version, "personId": clean("personId"),
                "propertyId": clean("propertyId"), "contractId": clean("contractId"),
            }))
            .map_err(invalid)?;
            apply_project_update(&state, &resolved, id.clone(), body).await?;
        }
    }
    let search = str_at(&command, "search").unwrap_or("").trim().to_owned();
    let page_index = command.get("page").and_then(Value::as_i64).unwrap_or(0).max(0);
    Ok(Json(opps_workbench(&state, &resolved, entity, &search, page_index, Some(id)).await?))
}

/// Listing Media's property rail: a page of properties with their photo counts, and the selected one.
pub(super) async fn listing_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OppsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let property = state.services().property();
    let page_index = query.page.as_deref().and_then(|page| page.trim().parse::<i64>().ok()).unwrap_or(0).max(0);
    let request = domain::PropertyAdminPageRequest {
        search: query.search.trim().to_owned(),
        page: page_index + 1,
        page_size: OPPS_PAGE_SIZE,
    };
    let page = to_json(property.admin_page(&request, &resolved.service).await.map_err(failed(&resolved))?);
    let rows = page.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    let row = |row: &Value| {
        json!({ "id": at(row, "id"), "name": at(row, "name"), "status": at(row, "status"),
                "slug": at(row, "slug"), "imageCount": at(row, "imageCount") })
    };
    let selected_id = query
        .selected
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .or_else(|| rows.first().and_then(|r| str_at(r, "id")).map(str::to_owned));
    let mut selected = selected_id.as_deref().and_then(|id| rows.iter().find(|r| str_at(r, "id") == Some(id)).cloned());
    if let (Some(id), None) = (selected_id.as_deref(), &selected) {
        // A selection off this page is read on its own; one that cannot be read is simply not selected.
        selected = property.admin_get(id, &resolved.service).await.ok().flatten().map(to_json);
    }
    Ok(Json(json!({ "listingMedia": {
        "properties": rows.iter().map(row).collect::<Vec<_>>(),
        "total": at(&page, "total"),
        "page": at(&page, "page"),
        "pageSize": at(&page, "pageSize"),
        "selectedId": selected.as_ref().map(|r| at(r, "id").clone()).or_else(|| rows.first().map(|r| at(r, "id").clone())),
        "selected": selected.as_ref().map(row),
    } })))
}

/// The role table as generic rows `{ id, cells: [role, account type, entitlement count] }`, as the relay flattened it.
pub(super) fn role_rows(roles: &Value) -> Vec<Value> {
    roles
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, role)| {
            let count = at(role, "entitlementCodes").as_array().map_or(0, Vec::len);
            let text = |key: &str| str_at(role, key).unwrap_or("").to_owned();
            json!({ "id": format!("item-{index}"), "cells": [text("roleCode"), text("accountType"), count.to_string()] })
        })
        .collect()
}
