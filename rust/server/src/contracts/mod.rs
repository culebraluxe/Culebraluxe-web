use crate::lookup::CoreEntityLookup;
use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{ContractDao, DbResult};
use domain::{
    Contract, ContractEffectiveState, ContractRole, ContractSummary, CreateContractFromFormRequest,
    ExecuteContractRequest, SaveContractDraftRequest, CONTRACT_FIRM_ROLE_CODES,
    CONTRACT_PERSON_ROLE_CODES,
};
use serde_json::json;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::BTreeMap;
use std::sync::Arc;

#[async_trait]
pub trait ContractRepository: Send + Sync {
    async fn get(&self, contract_id: &str) -> DbResult<Option<Contract>>;
    async fn list(&self) -> DbResult<Vec<ContractSummary>>;
    async fn list_for_process_instance(
        &self,
        process_instance_id: &str,
    ) -> DbResult<Vec<ContractSummary>>;
    async fn create_from_form(&self, request: &CreateContractFromFormRequest)
        -> DbResult<Contract>;
    async fn save_draft(&self, request: &SaveContractDraftRequest) -> DbResult<Contract>;
    async fn get_effective_state(
        &self,
        contract_id: &str,
    ) -> DbResult<Option<ContractEffectiveState>>;
    async fn execute(&self, request: &ExecuteContractRequest) -> DbResult<Option<Contract>>;
}

#[async_trait]
impl ContractRepository for ContractDao {
    async fn get(&self, contract_id: &str) -> DbResult<Option<Contract>> {
        ContractDao::get(self, contract_id).await
    }

    async fn list(&self) -> DbResult<Vec<ContractSummary>> {
        ContractDao::list(self).await
    }

    async fn list_for_process_instance(
        &self,
        process_instance_id: &str,
    ) -> DbResult<Vec<ContractSummary>> {
        ContractDao::list_for_process_instance(self, process_instance_id).await
    }

    async fn create_from_form(
        &self,
        request: &CreateContractFromFormRequest,
    ) -> DbResult<Contract> {
        ContractDao::create_from_form(self, request).await
    }

    async fn save_draft(&self, request: &SaveContractDraftRequest) -> DbResult<Contract> {
        ContractDao::save_draft(self, request).await
    }

    async fn get_effective_state(
        &self,
        contract_id: &str,
    ) -> DbResult<Option<ContractEffectiveState>> {
        ContractDao::get_effective_state(self, contract_id).await
    }

    async fn execute(&self, request: &ExecuteContractRequest) -> DbResult<Option<Contract>> {
        ContractDao::execute(self, request).await
    }
}

pub struct ContractService<R> {
    repository: R,
    lookup: Arc<dyn CoreEntityLookup>,
    runtime: ServiceRuntime,
}

impl<R: ContractRepository> ContractService<R> {
    pub fn new(
        repository: R,
        lookup: Arc<dyn CoreEntityLookup>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            lookup,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &self,
        contract_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<Contract>, CoreServiceError> {
        const OP: &str = "contract.get";
        let decision = authorize(
            &self.runtime,
            "contract",
            "contract.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(contract_id).await.map_err(Into::into);
        audit_result(&self.runtime, "contract", OP, context, decision, &result).await?;
        result
    }

    pub async fn list(
        &self,
        context: &ServiceContext,
    ) -> Result<Vec<ContractSummary>, CoreServiceError> {
        const OP: &str = "contract.list";
        let decision = authorize(
            &self.runtime,
            "contract",
            "contract.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.list().await.map_err(Into::into);
        audit_result(&self.runtime, "contract", OP, context, decision, &result).await?;
        result
    }

    pub async fn list_for_process_instance(
        &self,
        process_instance_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<ContractSummary>, CoreServiceError> {
        const OP: &str = "contract.listForProcessInstance";
        let decision = authorize(
            &self.runtime,
            "contract",
            "contract.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .list_for_process_instance(process_instance_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "contract", OP, context, decision, &result).await?;
        result
    }

    pub async fn create_from_form(
        &self,
        request: &CreateContractFromFormRequest,
        context: &ServiceContext,
    ) -> Result<Contract, CoreServiceError> {
        const OP: &str = "contract.createFromForm";
        let decision = authorize(
            &self.runtime,
            "contract",
            "contract.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            validate_role_codes(&request.roles)?;
            self.assert_parties(request, context).await?;
            let contract = self.repository.create_from_form(request).await?;
            self.emit_created("contract.created", &contract, context)
                .await?;
            Ok(contract)
        }
        .await;

        audit_result(&self.runtime, "contract", OP, context, decision, &result).await?;
        result
    }

    pub async fn save_draft(
        &self,
        request: &SaveContractDraftRequest,
        context: &ServiceContext,
    ) -> Result<Contract, CoreServiceError> {
        const OP: &str = "contract.saveDraft";
        let decision = authorize(
            &self.runtime,
            "contract",
            "contract.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            validate_role_codes(&request.roles)?;
            if let Some(existing) = self.repository.get(&request.contract_id).await? {
                if existing.status != "draft" {
                    return Err(CoreServiceError::business(
                        "CONTRACT_IMMUTABLE",
                        format!(
                            "Contract {} is {}; it is no longer an editable draft.",
                            request.contract_id, existing.status
                        ),
                    ));
                }
            }
            self.assert_parties(request, context).await?;
            let contract = self.repository.save_draft(request).await?;
            self.emit_created("contract.draft_saved", &contract, context)
                .await?;
            Ok(contract)
        }
        .await;

        audit_result(&self.runtime, "contract", OP, context, decision, &result).await?;
        result
    }

    pub async fn get_effective_state(
        &self,
        contract_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<ContractEffectiveState>, CoreServiceError> {
        const OP: &str = "contract.getEffectiveState";
        let decision = authorize(
            &self.runtime,
            "contract",
            "contract.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .get_effective_state(contract_id)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "contract", OP, context, decision, &result).await?;
        result
    }

    pub async fn execute(
        &self,
        request: &ExecuteContractRequest,
        context: &ServiceContext,
    ) -> Result<Contract, CoreServiceError> {
        const OP: &str = "contract.execute";
        let decision = authorize(
            &self.runtime,
            "contract",
            "contract.execute",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            let existing = self
                .repository
                .get(&request.contract_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "CONTRACT_NOT_FOUND",
                        format!("Contract not found: {}", request.contract_id),
                    )
                })?;

            if existing.status == "executed" {
                return Err(CoreServiceError::business(
                    "CONTRACT_ALREADY_EXECUTED",
                    format!("Contract {} is already executed.", request.contract_id),
                ));
            }
            if existing.status != "draft" {
                return Err(CoreServiceError::business(
                    "CONTRACT_NOT_EXECUTABLE",
                    format!(
                        "Contract {} is {}; only a draft can be executed.",
                        request.contract_id, existing.status
                    ),
                ));
            }

            let contract = self.repository.execute(request).await?.ok_or_else(|| {
                CoreServiceError::business(
                    "CONTRACT_NOT_FOUND",
                    format!("Contract not found: {}", request.contract_id),
                )
            })?;

            self.runtime
                .emit(
                    "contract.executed",
                    Some(contract.id.clone()),
                    BTreeMap::from([
                        ("contractId".into(), json!(contract.id.clone())),
                        ("contractType".into(), json!(contract.contract_type.clone())),
                        (
                            "evidenceDocumentId".into(),
                            json!(contract.evidence_document_id.clone()),
                        ),
                    ]),
                    context,
                )
                .await?;
            Ok(contract)
        }
        .await;

        audit_result(&self.runtime, "contract", OP, context, decision, &result).await?;
        result
    }

    async fn assert_parties(
        &self,
        request: &SaveContractDraftRequest,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        if !self
            .lookup
            .property_exists(&request.property_id, context)
            .await?
        {
            return Err(CoreServiceError::business(
                "PROPERTY_NOT_FOUND",
                format!("Property not found: {}", request.property_id),
            ));
        }

        for role in &request.roles {
            match role {
                ContractRole::Person { person_id, .. } => {
                    if !self.lookup.person_exists(person_id, context).await? {
                        return Err(CoreServiceError::business(
                            "PERSON_NOT_FOUND",
                            format!("Person not found: {person_id}"),
                        ));
                    }
                }
                ContractRole::Firm { firm_id, .. } => {
                    if !self.lookup.firm_exists(firm_id, context).await? {
                        return Err(CoreServiceError::business(
                            "FIRM_NOT_FOUND",
                            format!("Firm not found: {firm_id}"),
                        ));
                    }
                }
            }
        }

        if let Some(predecessor_id) = request.predecessor_contract_id.as_deref() {
            if self.repository.get(predecessor_id).await?.is_none() {
                return Err(CoreServiceError::business(
                    "PREDECESSOR_CONTRACT_NOT_FOUND",
                    format!("Predecessor Contract not found: {predecessor_id}"),
                ));
            }
        }

        Ok(())
    }

    async fn emit_created(
        &self,
        event_type: &'static str,
        contract: &Contract,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        self.runtime
            .emit(
                event_type,
                Some(contract.id.clone()),
                BTreeMap::from([
                    ("contractId".into(), json!(contract.id.clone())),
                    ("contractType".into(), json!(contract.contract_type.clone())),
                    (
                        "formTemplateId".into(),
                        json!(contract.form_template_id.clone()),
                    ),
                    ("propertyId".into(), json!(contract.property_id.clone())),
                    (
                        "predecessorContractId".into(),
                        json!(contract.predecessor_contract_id.clone()),
                    ),
                    ("roleCount".into(), json!(contract.roles.len())),
                ]),
                context,
            )
            .await?;
        Ok(())
    }
}

fn validate_role_codes(roles: &[ContractRole]) -> Result<(), CoreServiceError> {
    for role in roles {
        let code = role.role_code().trim().to_uppercase();
        if code.is_empty() {
            return Err(CoreServiceError::business(
                "ROLE_REQUIRED",
                "Every Contract identity mapping requires a Role code.",
            ));
        }
        let known = match role {
            ContractRole::Person { .. } => CONTRACT_PERSON_ROLE_CODES.contains(&code.as_str()),
            ContractRole::Firm { .. } => CONTRACT_FIRM_ROLE_CODES.contains(&code.as_str()),
        };
        if !known {
            return Err(CoreServiceError::business(
                "ROLE_UNKNOWN",
                format!("Unknown {} role code: {code}", role.scope()),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_contract_role_is_rejected() {
        let roles = vec![ContractRole::Person {
            person_id: "p".into(),
            role_code: "made_up".into(),
            ordinal: 0,
            snapshot_name: None,
            attributes: Default::default(),
        }];
        assert!(matches!(
            validate_role_codes(&roles),
            Err(CoreServiceError::Business {
                code: "ROLE_UNKNOWN",
                ..
            })
        ));
    }
}
