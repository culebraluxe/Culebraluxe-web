use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, IntakeDao};
use domain::{CatchupLeadRequest, CatchupLeadResult, WebsiteIntakeRequest, WebsiteIntakeResult};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait IntakeRepository: Send {
    async fn catchup_lead(&self, input: &CatchupLeadRequest) -> DbResult<CatchupLeadResult>;
    async fn website_intake(&self, input: &WebsiteIntakeRequest) -> DbResult<WebsiteIntakeResult>;
}

#[async_trait]
impl IntakeRepository for IntakeDao {
    async fn catchup_lead(&self, input: &CatchupLeadRequest) -> DbResult<CatchupLeadResult> {
        IntakeDao::catchup_lead(self, input).await
    }

    async fn website_intake(&self, input: &WebsiteIntakeRequest) -> DbResult<WebsiteIntakeResult> {
        IntakeDao::website_intake(self, input).await
    }
}

pub struct IntakeService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: IntakeRepository> IntakeService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn submit_website(
        &self,
        input: &WebsiteIntakeRequest,
        context: &ServiceContext,
    ) -> Result<WebsiteIntakeResult, CoreServiceError> {
        const OP: &str = "website.submitIntake";
        let decision = authorize(
            &self.runtime,
            "website",
            "website.intake.submit",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self
            .repository
            .website_intake(input)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "website", OP, context, decision, &result).await?;
        result
    }

    pub async fn submit_catchup(
        &self,
        input: &CatchupLeadRequest,
        context: &ServiceContext,
    ) -> Result<CatchupLeadResult, CoreServiceError> {
        const OP: &str = "catchup.createLead";
        let decision = authorize(
            &self.runtime,
            "crm",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self
            .repository
            .catchup_lead(input)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "crm", OP, context, decision, &result).await?;
        result
    }
}

// WEBSITE LEAD RULES — the form's validation, which the public site's handler applies before a lead is saved.
// (Ported from lib/website-intake.ts, where the TypeScript site enforced them.)

const WEBSITE_REQUEST_TYPES: &[&str] =
    &["private_viewing", "property_information", "general_enquiry"];
const WEBSITE_SERVICE_KEYS: &[&str] = &[
    "market-analysis",
    "property-evaluation",
    "comparable-research",
    "land-survey",
    "appraisal",
    "title-research",
    "consultation",
    "property-marketing",
];

fn nfkc_trim(value: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    value.nfkc().collect::<String>().trim().to_owned()
}

fn collapse_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn required_text(input: &serde_json::Value, key: &str, field: &str) -> Result<String, String> {
    let value = input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{field} is required."))?;
    let normalized = collapse_spaces(&nfkc_trim(value));
    if normalized.is_empty() {
        return Err(format!("{field} is required."));
    }
    Ok(normalized)
}

fn is_versioned_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| {
        (1..=5).contains(&id.get_version_num())
            && matches!(id.get_variant(), uuid::Variant::RFC4122)
    }) && value.len() == 36
}

fn is_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    let bad =
        |part: &str| part.is_empty() || part.contains(char::is_whitespace) || part.contains('@');
    if bad(local) || bad(domain) {
        return false;
    }
    domain
        .rsplit_once('.')
        .is_some_and(|(host, tld)| !host.is_empty() && !tld.is_empty())
}

/// A website form's fields -> the lead to save. `Ok(None)` is the honeypot: a bot filled the hidden `company`
/// field, which is answered as accepted and never saved. `Err` is the visitor-facing reason the form is invalid.
pub fn normalize_website_intake(
    input: &serde_json::Value,
) -> Result<Option<WebsiteIntakeRequest>, String> {
    let text = |key: &str| {
        input
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    };
    if !text("company").trim().is_empty() {
        return Ok(None);
    }
    let submission_id = required_text(input, "submissionId", "Submission ID")?;
    if !is_versioned_uuid(&submission_id) {
        return Err("Submission ID must be a UUID.".into());
    }
    let request_type = required_text(input, "requestType", "Request type")?;
    if !WEBSITE_REQUEST_TYPES.contains(&request_type.as_str()) {
        return Err("Request type is invalid.".into());
    }
    let property_scoped =
        request_type == "private_viewing" || request_type == "property_information";
    let property_id = if text("propertyId").trim().is_empty() {
        None
    } else {
        Some(required_text(input, "propertyId", "Property ID")?)
    };
    match (property_scoped, &property_id) {
        (true, None) => return Err("Property ID is required for this request type.".into()),
        (false, Some(_)) => return Err("A general enquiry must not include a property ID.".into()),
        _ => {}
    }
    if property_id
        .as_deref()
        .is_some_and(|id| !is_versioned_uuid(id))
    {
        return Err("Property ID must be a UUID.".into());
    }
    let display_name = required_text(input, "name", "Name")?;
    let email = required_text(input, "email", "Email")?.to_lowercase();
    if !is_email(&email) {
        return Err("Email is invalid.".into());
    }
    let message = Some(nfkc_trim(text("message"))).filter(|message| !message.is_empty());
    let service = Some(text("service").trim())
        .filter(|key| WEBSITE_SERVICE_KEYS.contains(key))
        .map(str::to_owned);
    if display_name.chars().count() > 200 {
        return Err("Name is too long.".into());
    }
    if email.chars().count() > 320 {
        return Err("Email is too long.".into());
    }
    if message.as_ref().is_some_and(|m| m.chars().count() > 4000) {
        return Err("Message is too long.".into());
    }
    Ok(Some(WebsiteIntakeRequest {
        submission_id,
        request_type,
        property_id,
        display_name,
        email,
        message,
        service,
    }))
}

#[cfg(test)]
mod website_rules {
    use super::normalize_website_intake;
    use serde_json::json;

    const ID: &str = "0c5b2f1e-8d7a-4c3b-9a1e-2f6d7c8b9a01";

    #[test]
    fn a_general_enquiry_is_normalized_and_a_property_one_needs_its_property() {
        let lead = normalize_website_intake(&json!({
            "submissionId": ID, "requestType": "general_enquiry", "name": "  Ana   Rivera ",
            "email": " Ana@Example.COM ", "message": "  Hola  ", "service": "appraisal",
        }))
        .unwrap()
        .unwrap();
        assert_eq!(
            (lead.display_name.as_str(), lead.email.as_str()),
            ("Ana Rivera", "ana@example.com")
        );
        assert_eq!(
            (lead.message.as_deref(), lead.service.as_deref()),
            (Some("Hola"), Some("appraisal"))
        );
        let missing = normalize_website_intake(&json!({
            "submissionId": ID, "requestType": "private_viewing", "name": "A", "email": "a@b.co",
        }));
        assert_eq!(
            missing.unwrap_err(),
            "Property ID is required for this request type."
        );
    }

    #[test]
    fn the_honeypot_is_accepted_unsaved_and_bad_input_is_refused() {
        assert!(normalize_website_intake(&json!({ "company": "bot" }))
            .unwrap()
            .is_none());
        let bad_email = normalize_website_intake(&json!({
            "submissionId": ID, "requestType": "general_enquiry", "name": "A", "email": "not-an-email",
        }));
        assert_eq!(bad_email.unwrap_err(), "Email is invalid.");
        let bad_id = normalize_website_intake(&json!({
            "submissionId": "nope", "requestType": "general_enquiry", "name": "A", "email": "a@b.co",
        }));
        assert_eq!(bad_id.unwrap_err(), "Submission ID must be a UUID.");
    }
}
