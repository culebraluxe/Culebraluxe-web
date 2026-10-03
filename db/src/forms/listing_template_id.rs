//! Moved from `forms.rs` (move only): LISTING_TEMPLATE_ID, SELLER_BROKER_NAME, FormRow, FormListRow, DealFactsRow, ParticipantSeedRow, EvidenceRow, DirectContextRow, SignerFormRow, SignerRow, BrokerRow, compact, string_map, map_form, map_list, role_for_form, FormDao.

#[allow(unused_imports)]
use super::*;

pub(super) const LISTING_TEMPLATE_ID: &str = "LISTING-01";
pub(super) const SELLER_BROKER_NAME: &str = "Lisa Penfield";

#[derive(Debug, FromRow)]
pub(super) struct FormRow {
    pub(super) id: String,
    pub(super) template_id: String,
    pub(super) template_version: i32,
    pub(super) deal_id: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) property_id: Option<String>,
    pub(super) contract_id: Option<String>,
    pub(super) status: String,
    pub(super) field_values: Value,
    pub(super) sections: Value,
    pub(super) created_by_user_id: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(super) struct FormListRow {
    pub(super) id: String,
    pub(super) template_id: String,
    pub(super) template_version: i32,
    pub(super) deal_id: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) property_id: Option<String>,
    pub(super) contract_id: Option<String>,
    pub(super) status: String,
    pub(super) field_values: Value,
    pub(super) sections: Value,
    pub(super) created_by_user_id: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) deal_label: Option<String>,
    pub(super) property_label: Option<String>,
    pub(super) client_name: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct DealFactsRow {
    pub(super) offer_price: Option<String>,
    pub(super) closing_date: Option<NaiveDate>,
    pub(super) financing_type: Option<String>,
    pub(super) property_name: Option<String>,
    pub(super) property_location: Option<String>,
    pub(super) client_name: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct ParticipantSeedRow {
    pub(super) role: String,
    pub(super) person_id: Option<String>,
    pub(super) display_name: String,
}

#[derive(Debug, FromRow)]
pub(super) struct EvidenceRow {
    pub(super) id: String,
    pub(super) property_id: Option<String>,
    pub(super) field_values: Value,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(super) struct DirectContextRow {
    pub(super) person_id: Option<String>,
    pub(super) property_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct SignerFormRow {
    pub(super) deal_id: Option<String>,
    pub(super) template_id: String,
    pub(super) status: String,
    pub(super) person_name: Option<String>,
    pub(super) resolved_person_id: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct SignerRow {
    pub(super) person_id: Option<String>,
    pub(super) display_name: String,
    pub(super) email: Option<String>,
    pub(super) role: String,
}

#[derive(Debug, FromRow)]
pub(super) struct BrokerRow {
    pub(super) person_id: Option<String>,
    pub(super) email: Option<String>,
}

pub(super) fn compact(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

pub(super) fn string_map(value: Value) -> BTreeMap<String, String> {
    serde_json::from_value(value).unwrap_or_default()
}

pub(super) fn map_form(row: FormRow) -> DbResult<FormInstance> {
    let status = FormInstanceStatus::try_from(row.status.as_str())
        .map_err(|error| DbFailure::schema_mismatch("form.map", error))?;

    Ok(FormInstance {
        id: row.id,
        template_id: row.template_id,
        template_version: row.template_version,
        deal_id: row.deal_id,
        person_id: row.person_id,
        property_id: row.property_id,
        contract_id: row.contract_id,
        status,
        field_values: string_map(row.field_values),
        sections: string_map(row.sections),
        created_by_user_id: row.created_by_user_id,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    })
}

pub(super) fn map_list(row: FormListRow) -> DbResult<FormInstanceListItem> {
    let instance = map_form(FormRow {
        id: row.id,
        template_id: row.template_id,
        template_version: row.template_version,
        deal_id: row.deal_id,
        person_id: row.person_id,
        property_id: row.property_id,
        contract_id: row.contract_id,
        status: row.status,
        field_values: row.field_values,
        sections: row.sections,
        created_by_user_id: row.created_by_user_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })?;

    Ok(FormInstanceListItem {
        instance,
        deal_label: compact(row.deal_label),
        property_label: compact(row.property_label),
        client_name: compact(row.client_name),
    })
}

pub(super) fn role_for_form(template_id: &str, role: &str) -> String {
    if template_id == LISTING_TEMPLATE_ID {
        if matches!(role, "owner" | "seller" | "SELLER_BROKER") {
            return "SELLER".into();
        }
    }

    match role {
        "client" => "BUYER".into(),
        "seller" | "owner" => "SELLER".into(),
        "" => "OTHER".into(),
        other => other.to_owned(),
    }
}

#[derive(Clone)]
pub struct FormDao {
    pub(super) db: Database,
}
