//! Listing Media's property rail — the page of properties with photo counts, the selected one, and the
//! generic role rows the security/roles table renders.
//!
//! Split out of `accounting_opps.rs` on 2026-09-28 (that file was 809 lines, nine over the 800-line rule).
//! The seam: this is the one part of the workbench that is about listing photos rather than the accounting
//! book or an OPPS record, and nothing in the parent's accounting or workbench code calls into it.

#[allow(unused_imports)]
use super::*;

/// Listing Media's property rail: a page of properties with their photo counts, and the selected one.
///
/// `pub`, not `pub(super)`: a grandchild's items are re-exported by a parent one level up (the convention in
/// `core/db/src/client.rs` and its children), and a `pub(super)` item cannot be re-exported any wider than
/// the module it lives in — which is what `E0364` says when this is written the other way round.
pub async fn listing_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OppsQuery>,
) -> Result<Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    let property = state.services().property();
    let page_index = query
        .page
        .as_deref()
        .and_then(|page| page.trim().parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let request = domain::PropertyAdminPageRequest {
        search: query.search.trim().to_owned(),
        page: page_index + 1,
        page_size: OPPS_PAGE_SIZE,
    };
    let page = to_json(
        property
            .admin_page(&request, &resolved.service)
            .await
            .map_err(failed(&resolved))?,
    );
    let rows = page
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let row = |row: &Value| {
        json!({ "id": at(row, "id"), "name": at(row, "name"), "status": at(row, "status"),
                "slug": at(row, "slug"), "imageCount": at(row, "imageCount") })
    };
    let selected_id = query
        .selected
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .or_else(|| {
            rows.first()
                .and_then(|r| str_at(r, "id"))
                .map(str::to_owned)
        });
    let mut selected = selected_id
        .as_deref()
        .and_then(|id| rows.iter().find(|r| str_at(r, "id") == Some(id)).cloned());
    if let (Some(id), None) = (selected_id.as_deref(), &selected) {
        // A selection off this page is read on its own; one that cannot be read is simply not selected.
        selected = property
            .admin_get(id, &resolved.service)
            .await
            .ok()
            .flatten()
            .map(to_json);
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
pub fn role_rows(roles: &Value) -> Vec<Value> {
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
