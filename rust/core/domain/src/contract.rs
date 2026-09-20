use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type ContractFacts = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ContractRole {
    Person {
        person_id: String,
        role_code: String,
        ordinal: i32,
        snapshot_name: Option<String>,
        attributes: ContractFacts,
    },
    Firm {
        firm_id: String,
        role_code: String,
        ordinal: i32,
        snapshot_name: Option<String>,
        attributes: ContractFacts,
    },
}

impl ContractRole {
    pub fn role_code(&self) -> &str {
        match self {
            Self::Person { role_code, .. } | Self::Firm { role_code, .. } => role_code,
        }
    }

    pub fn scope(&self) -> &'static str {
        match self {
            Self::Person { .. } => "contract_person",
            Self::Firm { .. } => "contract_firm",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub id: String,
    pub contract_type: String,
    pub form_template_id: String,
    pub source_form_instance_id: Option<String>,
    pub predecessor_contract_id: Option<String>,
    pub process_instance_id: Option<String>,
    pub property_id: String,
    pub roles: Vec<ContractRole>,
    pub facts: ContractFacts,
    pub status: String,
    pub executed_at: Option<String>,
    pub evidence_document_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractSummary {
    pub id: String,
    pub contract_type: String,
    pub form_template_id: String,
    pub status: String,
    pub property_id: String,
    pub predecessor_contract_id: Option<String>,
    pub process_instance_id: Option<String>,
    pub evidence_document_id: Option<String>,
    pub executed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractEffectiveState {
    pub contract_id: String,
    pub facts: ContractFacts,
    pub source_contract_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SaveContractDraftRequest {
    pub contract_id: String,
    pub contract_type: String,
    pub form_template_id: String,
    pub source_form_instance_id: Option<String>,
    pub predecessor_contract_id: Option<String>,
    pub process_instance_id: Option<String>,
    pub property_id: String,
    pub roles: Vec<ContractRole>,
    pub facts: ContractFacts,
}

pub type CreateContractFromFormRequest = SaveContractDraftRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteContractRequest {
    pub contract_id: String,
    pub evidence_document_id: Option<String>,
}

pub const CONTRACT_PERSON_ROLE_CODES: &[&str] = &[
    "BUYER",
    "SELLER",
    "SELLER_REPRESENTATIVE",
    "BUYER_BROKER",
    "SELLER_BROKER",
    "SELLER_SPOUSE",
    "CLOSING_NOTARY",
];

pub const CONTRACT_FIRM_ROLE_CODES: &[&str] = &[
    "BUYER",
    "SELLER",
    "BUYER_BROKERAGE",
    "SELLER_BROKERAGE",
    "ESCROW_HOLDER",
    "LENDER",
    "CLOSING_NOTARY",
];
