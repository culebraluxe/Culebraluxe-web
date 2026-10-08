//! Forms endpoints and shapes: the bridge, templates, fields, signers, actions, Grok fill, writes and preview.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FormsBridgeResponse {
    pub forms: FormsPage,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormsPage {
    pub items: Vec<FormItem>,
    pub selected: Option<FormItem>,
    pub template: Option<FormTemplate>,
    pub issued: Option<FormIssuedDocument>,
    pub signers: Vec<FormSigner>,
    pub signature: Option<FormSignatureState>,
    pub template_choices: Vec<FormTemplateChoice>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormItem {
    pub id: String,
    pub template_id: String,
    pub template_version: i32,
    pub template_name: String,
    pub active_version: i32,
    pub status: String,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
    pub contract_id: Option<String>,
    pub deal_label: Option<String>,
    pub property_label: Option<String>,
    pub client_name: Option<String>,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateChoice {
    pub id: String,
    pub display_name: String,
    pub active_version: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplate {
    pub id: String,
    pub version: i32,
    pub active_version: i32,
    pub display_name: String,
    pub document_type_label: String,
    pub rendering_title: String,
    pub presentation: String,
    pub fields: Vec<FormTemplateField>,
    pub sections: Vec<FormTemplateSection>,
    pub signature_groups: Vec<FormSignatureGroup>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateField {
    pub name: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub required: bool,
    pub options: Vec<String>,
    pub when: Option<FormWhen>,
    /// The template's own value; the screen does not ask for it.
    pub fixed: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormWhen {
    pub field: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateSection {
    pub name: String,
    pub label: String,
    pub editable: bool,
    pub segments: Vec<FormTemplateSegment>,
    pub when: Option<FormWhen>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormTemplateSegment {
    pub kind: String,
    pub text: Option<String>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormSignatureGroup {
    pub role: String,
    pub label: String,
    pub field: Option<String>,
    pub initials: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormSigner {
    pub slot_id: Option<String>,
    pub person_id: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormSignatureState {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormIssuedDocument {
    pub document_id: String,
    pub issued_version: i32,
    pub checksum: String,
    pub created_at: String,
    pub media_id: Option<String>,
}

pub struct FormsRead {
    pub record: Option<String>,
    pub deal_id: Option<String>,
    pub person_id: Option<String>,
    pub property_id: Option<String>,
}

impl FormsRead {
    pub fn list(
        deal_id: Option<String>,
        person_id: Option<String>,
        property_id: Option<String>,
    ) -> Self {
        Self {
            record: None,
            deal_id,
            person_id,
            property_id,
        }
    }

    pub fn record(id: impl Into<String>) -> Self {
        Self {
            record: Some(id.into()),
            deal_id: None,
            person_id: None,
            property_id: None,
        }
    }
}

impl Endpoint for FormsRead {
    const METHOD: Method = Method::Get;
    type Response = FormsBridgeResponse;

    fn path(&self) -> String {
        if let Some(id) = &self.record {
            return format!(
                "/api/portal/rust-ui/forms?screen=form-record&scope={}",
                encode(id)
            );
        }
        let mut path = "/api/portal/rust-ui/forms?screen=forms".to_string();
        if let Some(deal_id) = &self.deal_id {
            path.push_str(&format!("&dealId={}", encode(deal_id)));
        }
        if let Some(person_id) = &self.person_id {
            path.push_str(&format!("&personId={}", encode(person_id)));
        }
        if let Some(property_id) = &self.property_id {
            path.push_str(&format!("&propertyId={}", encode(property_id)));
        }
        path
    }
}

pub enum FormsAction {
    Create {
        template_id: String,
        deal_id: Option<String>,
        person_id: Option<String>,
        property_id: Option<String>,
        /// The seller as named on the contract: the server finds that person (or makes them).
        seller_name: Option<String>,
        /// The property's catastro number: the server finds that property.
        catastro: Option<String>,
    },
    Save {
        form_id: String,
        field_values: BTreeMap<String, String>,
        sections: BTreeMap<String, String>,
    },
    Issue {
        form_id: String,
        field_values: BTreeMap<String, String>,
        sections: BTreeMap<String, String>,
    },
    FillClient {
        form_id: String,
        seller_name: String,
    },
    SendSignature {
        form_id: String,
        field_values: BTreeMap<String, String>,
        sections: BTreeMap<String, String>,
    },
}

/// Grok's suggestion for the open form: the fields it sets, optionally new document prose, and a note. Nothing is saved.
pub struct FormsGrok {
    pub form_id: String,
    pub form_name: String,
    pub prompt: String,
    pub details_text: String,
    pub field_values: std::collections::BTreeMap<String, String>,
    pub fields: Vec<FormTemplateField>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormsGrokAnswer {
    pub field_values: std::collections::BTreeMap<String, String>,
    pub body: Option<String>,
    pub note: String,
}

impl Endpoint for FormsGrok {
    const METHOD: Method = Method::Post;
    type Response = FormsGrokAnswer;
    fn path(&self) -> String {
        "/api/portal/rust-ui/forms/grok".into()
    }
    fn body(&self) -> Option<serde_json::Value> {
        let fields: Vec<serde_json::Value> = self
            .fields
            .iter()
            .map(|field| serde_json::json!({ "name": field.name, "label": field.label, "type": field.field_type, "options": field.options }))
            .collect();
        Some(serde_json::json!({
            "formId": self.form_id,
            "formName": self.form_name,
            "prompt": self.prompt,
            "detailsText": self.details_text,
            "fieldValues": self.field_values,
            "fields": fields,
        }))
    }
}

pub struct FormsWrite {
    pub action: FormsAction,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormsWriteResponse {
    pub form_id: String,
    pub forms: FormsPage,
    pub message: Option<String>,
}

impl Endpoint for FormsWrite {
    const METHOD: Method = Method::Post;
    type Response = FormsWriteResponse;

    fn path(&self) -> String {
        "/api/portal/rust-ui/forms".into()
    }

    fn body(&self) -> Option<serde_json::Value> {
        Some(match &self.action {
            FormsAction::Create {
                template_id,
                deal_id,
                person_id,
                property_id,
                seller_name,
                catastro,
            } => serde_json::json!({
                "action": "create",
                "templateId": template_id,
                "dealId": deal_id,
                "personId": person_id,
                "propertyId": property_id,
                "sellerName": seller_name,
                "catastro": catastro,
            }),
            FormsAction::Save {
                form_id,
                field_values,
                sections,
            } => serde_json::json!({
                "action": "save",
                "formId": form_id,
                "fieldValues": field_values,
                "sections": sections,
            }),
            FormsAction::Issue {
                form_id,
                field_values,
                sections,
            } => serde_json::json!({
                "action": "issue",
                "formId": form_id,
                "fieldValues": field_values,
                "sections": sections,
            }),
            FormsAction::FillClient {
                form_id,
                seller_name,
            } => serde_json::json!({
                "action": "fillClient",
                "formId": form_id,
                "sellerName": seller_name,
            }),
            FormsAction::SendSignature {
                form_id,
                field_values,
                sections,
            } => serde_json::json!({
                "action": "sendSignature",
                "formId": form_id,
                "fieldValues": field_values,
                "sections": sections,
            }),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormPreviewResponse {
    pub data_uri: String,
    pub filename: String,
}

pub struct FormPreview {
    pub form_id: String,
    pub field_values: BTreeMap<String, String>,
    pub sections: BTreeMap<String, String>,
}

impl Endpoint for FormPreview {
    const METHOD: Method = Method::Post;
    type Response = FormPreviewResponse;

    fn path(&self) -> String {
        "/api/portal/rust-ui/forms/preview".into()
    }

    fn body(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "formId": self.form_id,
            "fieldValues": self.field_values,
            "sections": self.sections,
        }))
    }
}
