//! Clients, people, and the property admin reads and writes.

#[allow(unused_imports)]
use super::*;

pub(super) async fn clients(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ClientsQuery>,
) -> Result<Json<ApiSuccess<ClientPageResponse>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().clients();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 50);
    let search = query.search.unwrap_or_default();

    let value = if query.view.as_deref() == Some("admin") {
        ClientPageResponse::Admin(
            service
                .admin(
                    &ClientAdminPageRequest {
                        search,
                        page,
                        page_size,
                    },
                    &resolved.service,
                )
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?,
        )
    } else {
        let status = query
            .status
            .filter(|value| matches!(value.as_str(), "new" | "warm" | "active" | "referral"));
        let role = query
            .role
            .filter(|value| matches!(value.as_str(), "buyer" | "seller" | "both"));
        let sort = query
            .sort
            .filter(|value| matches!(value.as_str(), "name" | "created" | "recent"))
            .unwrap_or_else(|| "name".into());

        ClientPageResponse::Directory(
            service
                .directory(
                    &ClientDirectoryPageRequest {
                        search,
                        status,
                        role,
                        sort,
                        page,
                        page_size,
                    },
                    &resolved.service,
                )
                .await
                .map_err(|error| correlate(ApiError::from(error), &resolved))?,
        )
    };

    Ok(success(value, &resolved))
}

pub(super) async fn client_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
) -> Result<Json<ApiSuccess<Option<domain::ClientDetail>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().clients();
    let value = service
        .detail(&person_id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn client_agents(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<domain::AssignableAgent>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().clients();
    let value = service
        .agents(&resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn client_history(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(person_id): Path<String>,
    Query(query): Query<ClientHistoryQuery>,
) -> Result<Json<ApiSuccess<domain::ClientContactHistoryResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().clients();
    let value = service
        .history(
            &ClientHistoryRequest {
                person_id,
                page: query.page.unwrap_or(1).max(1),
                page_size: query.page_size.unwrap_or(20).clamp(1, 50),
                recent: query.recent.unwrap_or(false),
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn search_people(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PeopleSearchQuery>,
) -> Result<Json<ApiSuccess<Vec<domain::PersonSearchResult>>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().person();
    let value = service
        .search(
            &SearchPeopleRequest {
                query: query.query,
                limit: query.limit,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn person(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::Person>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().person();
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("PERSON_NOT_FOUND", format!("Person not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

pub(super) async fn update_person_admin(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdatePersonAdminBody>,
) -> Result<Json<ApiSuccess<domain::Person>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = apply_person_admin_update(&state, &resolved, id, body).await?;
    Ok(success(value, &resolved))
}

/// Shared by `/v1` and the portal page (`portal_bridge`), so the write and its cache refresh live once.
pub(in super::super) async fn apply_person_admin_update(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    id: String,
    body: UpdatePersonAdminBody,
) -> Result<domain::Person, ApiError> {
    let service = state.services().person();
    let (location, email, phone) = (body.location.clone(), body.email.clone(), body.phone.clone());
    let value = service
        .update_admin(
            &domain::UpdatePersonAdminRequest {
                person_id: id,
                display_name: body.display_name,
                civil_status: body.civil_status,
                status: body.status,
                company: body.company,
                location: body.location,
                email: body.email,
                phone: body.phone,
                manual_override: body.manual_override,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    state.services().clients().update_cached_person(&value);
    state.services().clients().update_cached_contact(&value.id, location.as_deref(), email.as_deref(), phone.as_deref());
    Ok(value)
}

pub(super) async fn properties_for_person(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::PersonPropertyContext>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().property();
    let value = service
        .for_person(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn property_admin_page(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PropertyAdminQuery>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminPage>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().property();
    let value = service
        .admin_page(
            &domain::PropertyAdminPageRequest {
                search: query.search,
                page: query.page.unwrap_or(1).max(1),
                page_size: query.page_size.unwrap_or(50).clamp(1, 100),
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

pub(super) async fn create_property_admin(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<CreatePropertyAdminBody>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminRecord>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = apply_property_admin_create(&state, &resolved, body).await?;
    Ok(success(value, &resolved))
}

/// Shared by `/v1` and the portal page (`portal_bridge`), so the write and its cache refresh live once.
pub(in super::super) async fn apply_property_admin_create(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    body: CreatePropertyAdminBody,
) -> Result<domain::PropertyAdminRecord, ApiError> {
    let service = state.services().property();
    let value = service
        .admin_create(
            &domain::CreatePropertyAdminRequest {
                name: body.name,
                property_type: body.property_type,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    service
        .warm_read_cache()
        .await
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    Ok(value)
}

pub(super) async fn property_admin_detail(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminRecord>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().property();
    let value = service
        .admin_get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("PROPERTY_NOT_FOUND", format!("Property not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}

pub(super) async fn save_property_admin(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SavePropertyAdminBody>,
) -> Result<Json<ApiSuccess<domain::PropertyAdminRecord>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = apply_property_admin_save(&state, &resolved, id, body).await?;
    Ok(success(value, &resolved))
}

/// Shared by `/v1` and the portal page (`portal_bridge`), so the write and its cache refresh live once.
pub(in super::super) async fn apply_property_admin_save(
    state: &ApiState,
    resolved: &ResolvedRequestContext,
    id: String,
    body: SavePropertyAdminBody,
) -> Result<domain::PropertyAdminRecord, ApiError> {
    let service = state.services().property();
    let value = service
        .admin_save(
            &domain::SavePropertyAdminRequest {
                property_id: id,
                name: body.name,
                slug: body.slug,
                status: body.status,
                featured: body.featured,
                is_active_listing: body.is_active_listing,
                is_published: body.is_published,
                property_type: body.property_type,
                has_ocean_view: body.has_ocean_view,
                has_bay_view: body.has_bay_view,
                has_beach_view: body.has_beach_view,
                has_harbor_view: body.has_harbor_view,
                has_island_view: body.has_island_view,
                has_mountain_view: body.has_mountain_view,
                has_sunrise_view: body.has_sunrise_view,
                has_sunset_view: body.has_sunset_view,
                has_water_access: body.has_water_access,
                has_beach_access: body.has_beach_access,
                has_pool: body.has_pool,
                has_generator: body.has_generator,
                has_solar: body.has_solar,
                is_furnished: body.is_furnished,
                is_gated: body.is_gated,
                list_price: body.list_price,
                original_list_price: body.original_list_price,
                location: body.location,
                address_line1: body.address_line1,
                street_number: body.street_number,
                street_name: body.street_name,
                unit_number: body.unit_number,
                city: body.city,
                state_or_province: body.state_or_province,
                neighborhood: body.neighborhood,
                postal_code: body.postal_code,
                country: body.country,
                iso_country_code: body.iso_country_code,
                latitude: body.latitude,
                longitude: body.longitude,
                bedrooms: body.bedrooms,
                bathrooms: body.bathrooms,
                bathrooms_full: body.bathrooms_full,
                bathrooms_half: body.bathrooms_half,
                square_feet: body.square_feet,
                lot_size: body.lot_size,
                lot_size_units: body.lot_size_units,
                lot_size_acres: body.lot_size_acres,
                lot_size_sqft: body.lot_size_sqft,
                road_frontage_feet: body.road_frontage_feet,
                road_surface_type: body.road_surface_type,
                lot_description: body.lot_description,
                utilities_notes: body.utilities_notes,
                catastro_number: body.catastro_number,
                buildability: body.buildability,
                slope_description: body.slope_description,
                pool_potential: body.pool_potential,
                road_adjacency: body.road_adjacency,
                utilities_availability: body.utilities_availability,
                hoa_status: body.hoa_status,
                view_description: body.view_description,
                year_built: body.year_built,
                stories: body.stories,
                parking_spaces: body.parking_spaces,
                short_description: body.short_description,
                editorial_description: body.editorial_description,
                public_remarks: body.public_remarks,
                seo_title: body.seo_title,
                seo_description: body.seo_description,
                hero_title: body.hero_title,
                tagline: body.tagline,
                architecture_notes: body.architecture_notes,
                amenities_notes: body.amenities_notes,
                lifestyle_notes: body.lifestyle_notes,
                listing_agent_name: body.listing_agent_name,
                listing_agent_email: body.listing_agent_email,
                listing_agent_phone: body.listing_agent_phone,
                listing_office: body.listing_office,
                legal_owner_name: body.legal_owner_name,
                listing_identifier: body.listing_identifier,
                registry_entry: body.registry_entry,
                finca_number: body.finca_number,
                registry_section: body.registry_section,
                seller_person_id: body.seller_person_id,
                archived: body.archived,
                stellar: body.stellar,
            },
            &resolved.service,
        )
        .await
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    service
        .warm_read_cache()
        .await
        .map_err(|error| correlate(ApiError::from(error), resolved))?;
    Ok(value)
}

pub(super) async fn property(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<domain::Property>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let service = state.services().property();
    let value = service
        .get(&id, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| {
            correlate(
                ApiError::not_found("PROPERTY_NOT_FOUND", format!("Property not found: {id}")),
                &resolved,
            )
        })?;
    Ok(success(value, &resolved))
}
