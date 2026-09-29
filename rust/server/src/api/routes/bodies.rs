//! Request bodies and queries the HTTP handlers accept, and the Mux client.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HealthResponse {
    pub(super) ok: bool,
    pub(super) service: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReadyResponse {
    pub(super) ok: bool,
    pub(super) database_target: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WhoAmI {
    pub(super) app_user_id: String,
    pub(super) display_name: String,
    pub(super) email: Option<String>,
    pub(super) account_type: String,
    pub(super) role_codes: Vec<String>,
    pub(super) authority_codes: Vec<String>,
    pub(super) entitlement_codes: Vec<String>,
    pub(super) person_id: Option<String>,
    pub(super) security_level: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GuestCodeRequestBody {
    pub(super) email: String,
    /// The visitor's address as the website saw it, for the per-IP limit.
    pub(super) client_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GuestCodeVerifyBody {
    pub(super) email: String,
    pub(super) code: String,
}

/// What the provider says about the proved identity; the identity itself arrives in the edge's identity headers.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GuestProvisionBody {
    pub(super) email: Option<String>,
    #[serde(default)]
    pub(super) email_verified: bool,
    pub(super) display_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuestCodeSent {
    pub(super) sent: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuestCodeVerified {
    pub(super) email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SetRoleEntitlementBody {
    pub(super) role_code: String,
    pub(super) action: String,
    pub(super) granted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SetUserPrimaryRoleBody {
    pub(super) app_user_id: String,
    pub(super) role_code: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct PeopleSearchQuery {
    #[serde(default)]
    pub(super) query: String,
    pub(super) limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClientsQuery {
    pub(super) view: Option<String>,
    pub(super) search: Option<String>,
    pub(super) status: Option<String>,
    pub(super) role: Option<String>,
    pub(super) sort: Option<String>,
    pub(super) page: Option<i64>,
    pub(super) page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClientHistoryQuery {
    pub(super) page: Option<i64>,
    pub(super) page_size: Option<i64>,
    pub(super) recent: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum ClientPageResponse {
    Directory(domain::ClientsPageResult),
    Admin(domain::ClientAdminPageResult),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommsPanelQuery {
    pub(super) moment_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommsTimelineQuery {
    pub(super) page: Option<i64>,
    pub(super) page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ActivityQuery {
    pub(super) limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IssuesQuery {
    pub(super) scope: Option<String>,
    pub(super) state: Option<String>,
    pub(super) page: Option<i64>,
    pub(super) page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RelationshipReviewQuery {
    pub(super) review_state: Option<String>,
    pub(super) search: Option<String>,
    pub(super) limit: Option<i64>,
    pub(super) offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RelationshipActionBody {
    pub(super) action: String,
    pub(super) id: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) confirm: Option<bool>,
    pub(super) source: Option<String>,
    pub(super) review_state: Option<String>,
    pub(super) limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TechCockpitQuery {
    pub(super) selected: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppDiagnosticEventBody {
    pub(super) kind: String,
    pub(super) operation: String,
    pub(super) message: String,
    pub(super) route: String,
    pub(super) level: String,
    pub(super) code: Option<String>,
    #[serde(default)]
    pub(super) meta: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct WhatsAppHandshakeQuery {
    #[serde(rename = "hub.mode")]
    pub(super) mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    pub(super) verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    pub(super) challenge: Option<String>,
}

/// The P&L's period. Both ends are required: see the handler for why there is no default.
#[derive(Debug, Deserialize)]
pub(super) struct AccountingPnlQuery {
    pub(super) from: String,
    pub(super) to: String,
}

/// The expense a caller is recording. The optional associations are `Option` because the form sends them only when the
/// expense belongs to something.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateExpenseBody {
    pub(super) vendor: String,
    pub(super) category: String,
    /// A decimal as a STRING, so the digits the operator typed reach Postgres unchanged. A JSON number here would have
    /// become a float and lost the cent.
    pub(super) amount: String,
    pub(super) expense_on: String,
    #[serde(default)]
    pub(super) memo: Option<String>,
    #[serde(default)]
    pub(super) deal_id: Option<String>,
    #[serde(default)]
    pub(super) property_id: Option<String>,
    #[serde(default)]
    pub(super) person_id: Option<String>,
}

/// The receivable a caller is recording.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateReceivableBody {
    #[serde(default)]
    pub(super) reference: Option<String>,
    pub(super) description: String,
    #[serde(default)]
    pub(super) category: String,
    pub(super) amount: String,
    pub(super) issued_on: String,
    #[serde(default)]
    pub(super) due_on: Option<String>,
    #[serde(default)]
    pub(super) deal_id: Option<String>,
    #[serde(default)]
    pub(super) property_id: Option<String>,
    #[serde(default)]
    pub(super) person_id: Option<String>,
}

/// The date a receivable was paid.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MarkReceivablePaidBody {
    pub(super) paid_on: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateFormBody {
    pub(super) template_id: String,
    pub(super) template_version: i32,
    pub(super) deal_id: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) property_id: Option<String>,
    #[serde(default)]
    pub(super) field_values: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) sections: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateFormBody {
    pub(super) field_values: Option<BTreeMap<String, String>>,
    pub(super) sections: Option<BTreeMap<String, String>>,
    pub(super) status: Option<String>,
    pub(super) contract_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateProjectBody {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) owner: Option<String>,
    pub(super) description: Option<String>,
    pub(super) areas: Option<Vec<String>>,
    pub(super) starts_at: Option<String>,
    pub(super) ends_at: Option<String>,
    pub(super) project_type: Option<String>,
    pub(super) playbook_id: Option<String>,
    pub(super) playbook_version: Option<i32>,
    pub(super) person_id: Option<String>,
    pub(super) property_id: Option<String>,
    pub(super) contract_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct UpdateProjectBody {
    pub(super) name: Option<String>,
    pub(super) owner: Option<String>,
    pub(super) status: Option<String>,
    pub(super) description: Option<String>,
    pub(super) areas: Option<Vec<String>>,
    pub(super) project_type: Option<String>,
    pub(super) playbook_id: Option<String>,
    pub(super) playbook_version: Option<i32>,
    pub(super) person_id: Option<String>,
    pub(super) property_id: Option<String>,
    pub(super) contract_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PropertyAdminQuery {
    #[serde(default)]
    pub(super) search: String,
    pub(super) page: Option<i64>,
    pub(super) page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct CreatePropertyAdminBody {
    pub(super) name: String,
    pub(super) property_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct SavePropertyAdminBody {
    pub(super) name: String,
    pub(super) slug: Option<String>,
    pub(super) status: String,
    pub(super) featured: bool,
    pub(super) is_active_listing: bool,
    pub(super) is_published: bool,
    pub(super) property_type: Option<String>,
    pub(super) has_ocean_view: bool,
    pub(super) has_bay_view: bool,
    pub(super) has_beach_view: bool,
    pub(super) has_harbor_view: bool,
    pub(super) has_island_view: bool,
    pub(super) has_mountain_view: bool,
    pub(super) has_sunrise_view: bool,
    pub(super) has_sunset_view: bool,
    pub(super) has_water_access: bool,
    pub(super) has_beach_access: bool,
    pub(super) has_pool: bool,
    pub(super) has_generator: bool,
    pub(super) has_solar: bool,
    pub(super) is_furnished: bool,
    pub(super) is_gated: bool,
    pub(super) list_price: Option<String>,
    pub(super) original_list_price: Option<String>,
    pub(super) location: Option<String>,
    pub(super) address_line1: Option<String>,
    pub(super) street_number: Option<String>,
    pub(super) street_name: Option<String>,
    pub(super) unit_number: Option<String>,
    pub(super) city: Option<String>,
    pub(super) state_or_province: Option<String>,
    pub(super) neighborhood: Option<String>,
    pub(super) postal_code: Option<String>,
    pub(super) country: Option<String>,
    pub(super) iso_country_code: Option<String>,
    pub(super) latitude: Option<String>,
    pub(super) longitude: Option<String>,
    pub(super) bedrooms: Option<String>,
    pub(super) bathrooms: Option<String>,
    pub(super) bathrooms_full: Option<String>,
    pub(super) bathrooms_half: Option<String>,
    pub(super) square_feet: Option<String>,
    pub(super) lot_size: Option<String>,
    pub(super) lot_size_units: Option<String>,
    pub(super) lot_size_acres: Option<String>,
    pub(super) lot_size_sqft: Option<String>,
    pub(super) road_frontage_feet: Option<String>,
    pub(super) road_surface_type: Option<String>,
    pub(super) lot_description: Option<String>,
    pub(super) utilities_notes: Option<String>,
    pub(super) catastro_number: Option<String>,
    pub(super) buildability: Option<String>,
    pub(super) slope_description: Option<String>,
    pub(super) pool_potential: Option<String>,
    pub(super) road_adjacency: Option<String>,
    pub(super) utilities_availability: Option<String>,
    pub(super) hoa_status: Option<String>,
    pub(super) view_description: Option<String>,
    pub(super) year_built: Option<String>,
    pub(super) stories: Option<String>,
    pub(super) parking_spaces: Option<String>,
    pub(super) short_description: Option<String>,
    pub(super) editorial_description: Option<String>,
    pub(super) public_remarks: Option<String>,
    pub(super) seo_title: Option<String>,
    pub(super) seo_description: Option<String>,
    pub(super) hero_title: Option<String>,
    pub(super) tagline: Option<String>,
    pub(super) architecture_notes: Option<String>,
    pub(super) amenities_notes: Option<String>,
    pub(super) lifestyle_notes: Option<String>,
    pub(super) listing_agent_name: Option<String>,
    pub(super) listing_agent_email: Option<String>,
    pub(super) listing_agent_phone: Option<String>,
    pub(super) listing_office: Option<String>,
    pub(super) legal_owner_name: Option<String>,
    pub(super) listing_identifier: Option<String>,
    pub(super) registry_entry: Option<String>,
    pub(super) finca_number: Option<String>,
    pub(super) registry_section: Option<String>,
    pub(super) seller_person_id: Option<String>,
    pub(super) archived: bool,
    #[serde(default)]
    pub(super) stellar: domain::PropertyStellarDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AttachPropertyVideoBody {
    pub(super) role: String,
    pub(super) mux_asset_id: String,
    pub(super) mux_playback_id: String,
    pub(super) duration_seconds: Option<String>,
    pub(super) aspect_ratio: Option<String>,
    pub(super) caption: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreatePropertyVideoUploadBody {
    pub(super) cors_origin: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalizePropertyVideoUploadBody {
    pub(super) role: String,
    pub(super) caption: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct UpdatePersonAdminBody {
    pub(super) display_name: String,
    pub(super) civil_status: Option<String>,
    pub(super) status: String,
    pub(super) company: Option<String>,
    #[serde(default)]
    pub(super) location: Option<String>,
    #[serde(default)]
    pub(super) email: Option<String>,
    #[serde(default)]
    pub(super) phone: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateWbsBody {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) notes: Option<String>,
    pub(super) category: String,
    pub(super) project_id: Option<String>,
    pub(super) parent_id: Option<String>,
    pub(super) due_at: Option<String>,
    pub(super) planned_start: Option<String>,
    pub(super) planned_finish: Option<String>,
    pub(super) owner: Option<String>,
    pub(super) order: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AddWbsDependencyBody {
    pub(super) source_id: String,
    pub(super) target_id: String,
    pub(super) kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct UpdateWbsBody {
    pub(super) title: Option<String>,
    pub(super) notes: Option<String>,
    pub(super) status: Option<String>,
    #[serde(default, deserialize_with = "present_nullable_date")]
    pub(super) due_at: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_nullable_date")]
    pub(super) planned_start: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_nullable_date")]
    pub(super) planned_finish: Option<Option<String>>,
    pub(super) owner: Option<Option<String>>,
}

pub(super) fn present_nullable_date<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod wbs_schedule_body_tests {
    use super::super::UpdateWbsBody;

    #[test]
    fn patch_distinguishes_omitted_planned_date_from_explicit_clear() {
        let omitted: UpdateWbsBody = serde_json::from_str("{}").unwrap();
        let cleared: UpdateWbsBody = serde_json::from_str(r#"{"plannedStart":null}"#).unwrap();
        let changed: UpdateWbsBody = serde_json::from_str(r#"{"plannedStart":"2026-09-10"}"#).unwrap();
        assert_eq!(omitted.planned_start, None);
        assert_eq!(cleared.planned_start, Some(None));
        assert_eq!(changed.planned_start, Some(Some("2026-09-10".into())));
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct AppleReminderBody {
    pub(super) alert: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BindVaultFormContractBody {
    pub(super) contract_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RouteProjectWorkBody {
    pub(super) title: String,
    pub(super) notes: String,
    pub(super) status: String,
    pub(super) due_at: Option<String>,
    pub(super) owner: Option<String>,
    pub(super) destination: String,
    pub(super) start_at: Option<String>,
    pub(super) end_at: Option<String>,
    pub(super) location: Option<String>,
    pub(super) alert: Option<bool>,
}

pub(in super::super) fn mux_video() -> Result<MuxClient, ApiError> {
    let config = MuxConfig::from_env().map_err(|message| {
        ApiError::from(CoreServiceError::business("MUX_NOT_CONFIGURED", message))
    })?;
    MuxClient::new(config).map_err(|error| {
        ApiError::from(CoreServiceError::business(
            "MUX_NOT_CONFIGURED",
            error.message,
        ))
    })
}
