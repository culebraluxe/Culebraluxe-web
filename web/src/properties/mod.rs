use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use chrono::NaiveDate;
use db::{Database, DbResult, PropertyDao};
use model::{
    CreatePropertyAdminRequest, FindPropertyByAddressRequest, PersonPropertyContext, Property,
    PropertyAdminPage, PropertyAdminPageRequest, PropertyAdminRecord, PropertyForPerson,
    SavePropertyAdminRequest, SetPropertyDisplayNameRequest, SetPropertyListingTypeRequest,
    SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};
use serde_json::json;
use services::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;

#[async_trait]
pub trait PropertyRepository: Send + Sync {
    fn database(&self) -> Option<Database> {
        None
    }
    async fn get(&self, property_id: &str) -> DbResult<Option<Property>>;
    async fn find_by_address(
        &self,
        request: &FindPropertyByAddressRequest,
    ) -> DbResult<Option<Property>>;
    async fn for_person(&self, person_id: &str) -> DbResult<PersonPropertyContext>;
    async fn upsert_for_person(
        &self,
        request: &UpsertPropertyForPersonRequest,
    ) -> DbResult<PropertyForPerson>;
    async fn set_display_name(
        &self,
        request: &SetPropertyDisplayNameRequest,
    ) -> DbResult<Option<Property>>;
    async fn set_status(&self, request: &SetPropertyStatusRequest) -> DbResult<Option<Property>>;
    async fn set_listing_type(&self, request: &SetPropertyListingTypeRequest) -> DbResult<()>;
    async fn admin_page(&self, request: &PropertyAdminPageRequest) -> DbResult<PropertyAdminPage>;
    async fn admin_get(&self, property_id: &str) -> DbResult<Option<PropertyAdminRecord>>;
    async fn admin_create(
        &self,
        request: &CreatePropertyAdminRequest,
    ) -> DbResult<PropertyAdminRecord>;
    async fn admin_save(
        &self,
        request: &SavePropertyAdminRequest,
    ) -> DbResult<Option<PropertyAdminRecord>>;
    async fn merge_parcel_record(
        &self,
        target_id: &str,
        catastro: &str,
    ) -> DbResult<Option<String>>;
}

#[async_trait]
impl PropertyRepository for PropertyDao {
    fn database(&self) -> Option<Database> {
        Some(PropertyDao::database(self))
    }
    async fn get(&self, property_id: &str) -> DbResult<Option<Property>> {
        db::retrying_read!(PropertyDao::get(self, property_id))
    }

    async fn find_by_address(
        &self,
        request: &FindPropertyByAddressRequest,
    ) -> DbResult<Option<Property>> {
        db::retrying_read!(PropertyDao::find_by_address(self, request))
    }

    async fn for_person(&self, person_id: &str) -> DbResult<PersonPropertyContext> {
        db::retrying_read!(PropertyDao::for_person(self, person_id))
    }

    // The three below write. They are deliberately not retried: a transient failure here can happen *after* the write
    // landed, and repeating it would either duplicate a link or silently re-apply a status change. Reads above are
    // repeatable; these are not.
    async fn upsert_for_person(
        &self,
        request: &UpsertPropertyForPersonRequest,
    ) -> DbResult<PropertyForPerson> {
        PropertyDao::upsert_for_person(self, request).await
    }

    async fn set_display_name(
        &self,
        request: &SetPropertyDisplayNameRequest,
    ) -> DbResult<Option<Property>> {
        PropertyDao::set_display_name(self, request).await
    }

    async fn set_status(&self, request: &SetPropertyStatusRequest) -> DbResult<Option<Property>> {
        PropertyDao::set_status(self, request).await
    }

    async fn set_listing_type(&self, request: &SetPropertyListingTypeRequest) -> DbResult<()> {
        PropertyDao::set_listing_type(self, request).await
    }

    async fn admin_page(&self, request: &PropertyAdminPageRequest) -> DbResult<PropertyAdminPage> {
        db::retrying_read!(PropertyDao::admin_page(self, request))
    }

    async fn admin_get(&self, property_id: &str) -> DbResult<Option<PropertyAdminRecord>> {
        db::retrying_read!(PropertyDao::admin_get(self, property_id))
    }

    async fn admin_create(
        &self,
        request: &CreatePropertyAdminRequest,
    ) -> DbResult<PropertyAdminRecord> {
        PropertyDao::admin_create(self, request).await
    }

    async fn merge_parcel_record(
        &self,
        target_id: &str,
        catastro: &str,
    ) -> DbResult<Option<String>> {
        PropertyDao::merge_parcel_record(self, target_id, catastro).await
    }

    async fn admin_save(
        &self,
        request: &SavePropertyAdminRequest,
    ) -> DbResult<Option<PropertyAdminRecord>> {
        PropertyDao::admin_save(self, request).await
    }
}

pub struct PropertyService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: PropertyRepository> PropertyService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<Property>, CoreServiceError> {
        const OP: &str = "property.get";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(property_id).await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn find_by_address(
        &self,
        request: &FindPropertyByAddressRequest,
        context: &ServiceContext,
    ) -> Result<Option<Property>, CoreServiceError> {
        const OP: &str = "property.findByAddress";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .find_by_address(request)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn for_person(
        &self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<PersonPropertyContext, CoreServiceError> {
        const OP: &str = "property.forPerson";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .for_person(person_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn upsert_for_person(
        &self,
        request: &UpsertPropertyForPersonRequest,
        context: &ServiceContext,
    ) -> Result<PropertyForPerson, CoreServiceError> {
        const OP: &str = "property.upsertForPerson";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            let linked = self.repository.upsert_for_person(request).await?;
            self.runtime
                .emit(
                    "property.person_context_upserted",
                    Some(linked.property.id.clone()),
                    BTreeMap::from([
                        ("personId".into(), json!(request.person_id.clone())),
                        ("propertyId".into(), json!(linked.property.id.clone())),
                        ("relation".into(), json!(linked.relation.as_str())),
                        (
                            "sourceType".into(),
                            json!(request
                                .source_type
                                .clone()
                                .unwrap_or_else(|| "manual".into())),
                        ),
                    ]),
                    context,
                )
                .await?;
            Ok(linked)
        })
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn set_display_name(
        &self,
        request: &SetPropertyDisplayNameRequest,
        context: &ServiceContext,
    ) -> Result<Property, CoreServiceError> {
        const OP: &str = "property.setDisplayName";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            let property = self
                .repository
                .set_display_name(request)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "PROPERTY_NOT_FOUND",
                        format!("Property not found: {}", request.property_id),
                    )
                })?;
            self.runtime
                .emit(
                    "property.display_name_changed",
                    Some(property.id.clone()),
                    BTreeMap::from([
                        ("propertyId".into(), json!(property.id.clone())),
                        ("displayName".into(), json!(property.display_name.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(property)
        })
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn admin_page(
        &self,
        request: &PropertyAdminPageRequest,
        context: &ServiceContext,
    ) -> Result<PropertyAdminPage, CoreServiceError> {
        const OP: &str = "property.adminPage";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .admin_page(request)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn admin_get(
        &self,
        property_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<PropertyAdminRecord>, CoreServiceError> {
        const OP: &str = "property.adminGet";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .admin_get(property_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    /// FIND by catastro: the other record for this parcel is merged into this one — every field this record lacks is
    /// filled from it, what hangs off it moves here, and it is deleted. `None` when no other record has that number.
    pub async fn merge_parcel_record(
        &self,
        target_id: &str,
        catastro: &str,
        context: &ServiceContext,
    ) -> Result<Option<String>, CoreServiceError> {
        const OP: &str = "property.mergeParcelRecord";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            let merged = self
                .repository
                .merge_parcel_record(target_id.trim(), catastro.trim())
                .await?;
            if let Some(name) = &merged {
                self.runtime
                    .emit(
                        "property.parcel_merged",
                        Some(target_id.to_owned()),
                        BTreeMap::from([
                            ("propertyId".into(), json!(target_id)),
                            ("catastro".into(), json!(catastro)),
                            ("mergedName".into(), json!(name)),
                        ]),
                        context,
                    )
                    .await?;
            }
            Ok(merged)
        })
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn admin_create(
        &self,
        request: &CreatePropertyAdminRequest,
        context: &ServiceContext,
    ) -> Result<PropertyAdminRecord, CoreServiceError> {
        const OP: &str = "property.adminCreate";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            if request.name.trim().is_empty() {
                return Err(CoreServiceError::business(
                    "PROPERTY_NAME_REQUIRED",
                    "Property name is required.",
                ));
            }
            let property = self.repository.admin_create(request).await?;
            self.runtime
                .emit(
                    "property.created",
                    Some(property.id.clone()),
                    BTreeMap::from([
                        ("propertyId".into(), json!(property.id.clone())),
                        ("name".into(), json!(property.name.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(property)
        })
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn admin_save(
        &self,
        request: &SavePropertyAdminRequest,
        context: &ServiceContext,
    ) -> Result<PropertyAdminRecord, CoreServiceError> {
        const OP: &str = "property.adminSave";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            // Loaded to REFUSE a save against a property that does not exist — the value itself is not consulted
            // beyond that, which is why it is bound with the underscore rather than passed to the validator.
            let _current = self
                .repository
                .admin_get(&request.property_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "PROPERTY_NOT_FOUND",
                        format!("Property not found: {}", request.property_id),
                    )
                })?;
            validate_admin_save(request)?;
            let property = self.repository.admin_save(request).await?.ok_or_else(|| {
                CoreServiceError::business(
                    "PROPERTY_NOT_FOUND",
                    format!("Property not found: {}", request.property_id),
                )
            })?;
            self.runtime
                .emit(
                    "property.admin_saved",
                    Some(property.id.clone()),
                    BTreeMap::from([
                        ("propertyId".into(), json!(property.id.clone())),
                        ("name".into(), json!(property.name.clone())),
                        ("status".into(), json!(property.status.clone())),
                        ("published".into(), json!(property.is_published)),
                        ("activeListing".into(), json!(property.is_active_listing)),
                    ]),
                    context,
                )
                .await?;
            Ok(property)
        })
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn set_status(
        &self,
        request: &SetPropertyStatusRequest,
        context: &ServiceContext,
    ) -> Result<Property, CoreServiceError> {
        const OP: &str = "property.setStatus";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = db::service_mutation(self.repository.database(), async {
            let property = self.repository.set_status(request).await?.ok_or_else(|| {
                CoreServiceError::business(
                    "PROPERTY_NOT_FOUND",
                    format!("Property not found: {}", request.property_id),
                )
            })?;
            self.runtime
                .emit(
                    "property.status_changed",
                    Some(property.id.clone()),
                    BTreeMap::from([
                        ("propertyId".into(), json!(property.id.clone())),
                        ("status".into(), json!(property.status.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(property)
        })
        .await;
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }
    pub async fn set_listing_type(
        &self,
        request: &SetPropertyListingTypeRequest,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "property.setListingType";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = db::service_mutation(self.repository.database(), async {
            if let Some(value) = request
                .listing_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                const LISTING_TYPES: &[&str] = &["Exclusive Right to Sell", "Exclusive Agency"];
                if !LISTING_TYPES.contains(&value) {
                    return Err(CoreServiceError::business(
                        "PROPERTY_LISTING_TYPE_INVALID",
                        "Listing type must be Exclusive Right to Sell or Exclusive Agency.",
                    ));
                }
            }
            self.repository.set_listing_type(request).await?;
            self.runtime
                .emit(
                    "property.listing_type_changed",
                    Some(request.property_id.clone()),
                    BTreeMap::from([
                        ("propertyId".into(), json!(request.property_id.clone())),
                        ("listingType".into(), json!(request.listing_type.clone())),
                    ]),
                    context,
                )
                .await?;
            Ok(())
        })
        .await;

        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }
}

impl PropertyService<PropertyDao> {
    pub async fn warm_read_cache(&self) -> Result<usize, CoreServiceError> {
        self.repository.warm_read_cache().await.map_err(Into::into)
    }
}

fn compact_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn parse_non_negative(value: Option<&str>, label: &'static str) -> Result<(), CoreServiceError> {
    let Some(raw) = compact_text(value) else {
        return Ok(());
    };
    let parsed = raw.parse::<f64>().map_err(|_| {
        CoreServiceError::business(
            "PROPERTY_NUMBER_INVALID",
            format!("{label} must be a number."),
        )
    })?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(CoreServiceError::business(
            "PROPERTY_NUMBER_INVALID",
            format!("{label} must be zero or greater."),
        ));
    }
    Ok(())
}

fn parse_range(
    value: Option<&str>,
    label: &'static str,
    min: f64,
    max: f64,
) -> Result<(), CoreServiceError> {
    let Some(raw) = compact_text(value) else {
        return Ok(());
    };
    let parsed = raw.parse::<f64>().map_err(|_| {
        CoreServiceError::business(
            "PROPERTY_NUMBER_INVALID",
            format!("{label} must be a number."),
        )
    })?;
    if !parsed.is_finite() || parsed < min || parsed > max {
        return Err(CoreServiceError::business(
            "PROPERTY_NUMBER_INVALID",
            format!("{label} must be between {min} and {max}."),
        ));
    }
    Ok(())
}

fn parse_iso_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn validate_admin_save(request: &SavePropertyAdminRequest) -> Result<(), CoreServiceError> {
    if request.name.trim().is_empty() {
        return Err(CoreServiceError::business(
            "PROPERTY_NAME_REQUIRED",
            "Property name is required.",
        ));
    }

    const STATUSES: &[&str] = &[
        "prospect",
        "coming_soon",
        "active",
        "off_market",
        "under_contract",
        "sold",
        "archived",
    ];
    if !STATUSES.contains(&request.status.trim()) {
        return Err(CoreServiceError::business(
            "PROPERTY_STATUS_INVALID",
            "Property status is invalid.",
        ));
    }
    // NO WORKFLOW-OWNERSHIP CONSTRAINT.
    //
    // There used to be a guard here refusing any status change touching `under_contract` or `sold`, on the grounds that
    // the transaction workflow owns those. It was removed because it protected nothing: the workflow sets states
    // through its own path either way, and what this actually did was refuse the owner of the business the ability to
    // correct a listing — a sold Property could not be archived, which surfaced as `internal_error` with no reason.
    // A rule that only ever says no to the person in charge is not a rule, it is a trap.

    // ARCHIVING IS TWO THINGS AND THEY MUST AGREE: `status = 'archived'` is what a person chose, `archived_at` is what
    // every public read filters on. The DAO derives the timestamp from the status (see `admin_save`), so the two can
    // never disagree — a listing cannot say archived and stay published.

    if let Some(slug) = compact_text(request.slug.as_deref()) {
        let valid = slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && !slug.starts_with('-')
            && !slug.ends_with('-')
            && !slug.contains("--");
        if !valid {
            return Err(CoreServiceError::business(
                "PROPERTY_SLUG_INVALID",
                "Slug must use lowercase letters, numbers, and single hyphens.",
            ));
        }
    }

    for (value, label) in [
        (request.list_price.as_deref(), "List price"),
        (request.bedrooms.as_deref(), "Bedrooms"),
        (request.bathrooms.as_deref(), "Bathrooms"),
        (request.bathrooms_full.as_deref(), "Full bathrooms"),
        (request.bathrooms_half.as_deref(), "Half bathrooms"),
        (request.square_feet.as_deref(), "Square feet"),
        (request.lot_size.as_deref(), "Lot size"),
        (request.lot_size_acres.as_deref(), "Lot acres"),
        (request.lot_size_sqft.as_deref(), "Lot square feet"),
        (request.road_frontage_feet.as_deref(), "Road frontage"),
        (
            request.original_list_price.as_deref(),
            "Original list price",
        ),
        (request.year_built.as_deref(), "Year built"),
        (request.stories.as_deref(), "Stories"),
        (request.parking_spaces.as_deref(), "Parking spaces"),
        (request.stellar.tax_year.as_deref(), "Tax year"),
        (request.stellar.annual_tax.as_deref(), "Annual tax"),
    ] {
        parse_non_negative(value, label)?;
    }
    if let Some(hoa) = compact_text(request.hoa_status.as_deref()) {
        if !matches!(hoa, "Yes" | "No") {
            return Err(CoreServiceError::business(
                "PROPERTY_HOA_STATUS_INVALID",
                "HOA status must be Yes, No, or unknown.",
            ));
        }
    }
    if let Some(raw) = compact_text(request.stellar.total_area_sqft.as_deref()) {
        let total_area = raw.parse::<f64>().map_err(|_| {
            CoreServiceError::business("PROPERTY_NUMBER_INVALID", "Total area must be a number.")
        })?;
        if !total_area.is_finite() || total_area <= 0.0 {
            return Err(CoreServiceError::business(
                "PROPERTY_NUMBER_INVALID",
                "Total area must be greater than zero.",
            ));
        }
    }
    parse_range(request.latitude.as_deref(), "Latitude", -90.0, 90.0)?;
    parse_range(request.longitude.as_deref(), "Longitude", -180.0, 180.0)?;

    if let Some(year) = compact_text(request.stellar.tax_year.as_deref()) {
        let parsed = year.parse::<i32>().map_err(|_| {
            CoreServiceError::business(
                "PROPERTY_TAX_YEAR_INVALID",
                "Tax year must be a whole year.",
            )
        })?;
        if !(1800..=2200).contains(&parsed) {
            return Err(CoreServiceError::business(
                "PROPERTY_TAX_YEAR_INVALID",
                "Tax year must be between 1800 and 2200.",
            ));
        }
    }

    let contract_raw = compact_text(request.stellar.listing_contract_date.as_deref());
    let expiration_raw = compact_text(request.stellar.expiration_date.as_deref());
    let contract = match contract_raw {
        Some(value) => Some(parse_iso_date(value).ok_or_else(|| {
            CoreServiceError::business(
                "PROPERTY_DATE_INVALID",
                "Listing contract date must use YYYY-MM-DD.",
            )
        })?),
        None => None,
    };
    let expiration = match expiration_raw {
        Some(value) => Some(parse_iso_date(value).ok_or_else(|| {
            CoreServiceError::business(
                "PROPERTY_DATE_INVALID",
                "Expiration date must use YYYY-MM-DD.",
            )
        })?),
        None => None,
    };
    if let (Some(contract), Some(expiration)) = (contract, expiration) {
        if expiration < contract {
            return Err(CoreServiceError::business(
                "PROPERTY_DATE_INVALID",
                "Expiration must be on or after the listing contract date.",
            ));
        }
    }

    Ok(())
}
