//! The SIG.PROVIDER / SIG.RECONCILE harness: the real `SignatureService` over a store.
//!
//! `SignatureService` owns the lifecycle every SIG contract is about — send, refresh, cancel, webhook delivery,
//! reconciliation — and production wires it to a real `SignatureDao` plus a real external provider. A test that
//! described its own copy of that logic would prove nothing, so this harness runs the REAL service and fakes only
//! at the seams production itself defines:
//!
//! - the store is faked at [`web::signature::SignatureRepository`], the trait `SignatureDao` implements;
//! - the external provider is [`crate::providers::FakeSignatureProvider`], the fake for
//!   `services::SignatureProvider` — the same trait the BoldSign adapter implements;
//! - infrastructure is the real service boundary (`ServiceInfrastructure`) with capturing, non-writing ports.
//!
//! Nothing here opens a socket, writes to a database, or reaches a live provider.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use db::{DbFailure, DbResult};
use model::{
    ApplySignatureStatusRequest, SendSignatureRequest, SignatureArtifactDownload,
    SignatureCommandOutcome, SignatureCommandResult, SignatureRequest, SignatureRequestResult,
    SignatureRequestStatus, SignatureStatusResult,
};
use serde_json::json;
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use web::signature::{SignatureRepository, SignatureService};

use crate::providers::FakeSignatureProvider;

/// Production infrastructure with capturing (non-writing) audit/event ports and the default authorization port —
/// commands from a non-GUEST principal are admitted, matching how the service tests run.
pub fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

/// The context a back-office command (refresh, cancel) arrives under: a signed-in USER principal.
pub fn user_context(app_user_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(app_user_id.to_owned()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "sig-contract".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: app_user_id.to_owned(),
            level: "USER".into(),
            role_codes: vec!["user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

#[derive(Clone)]
struct StoredRequest {
    request: SignatureRequest,
}

/// The in-memory store standing in for `luxesign_request`, wired to the REAL `SignatureService`.
///
/// The fake records every mutating call so a test can assert that an error from the provider short-circuits BEFORE
/// any state change — the invariant the "provider error mapping" and "download failure rolls back" contracts
/// depend on.
#[derive(Clone)]
pub struct SignatureHarness {
    requests: Arc<Mutex<HashMap<String, StoredRequest>>>,
    document_signed: Arc<Mutex<bool>>,
    reconciled_bytes: Arc<Mutex<Vec<Vec<u8>>>>,
    calls: Arc<Mutex<Vec<String>>>,
    provider: Arc<FakeSignatureProvider>,
}

impl Default for SignatureHarness {
    fn default() -> Self {
        Self::new(FakeSignatureProvider::accepting())
    }
}

impl SignatureHarness {
    pub fn new(provider: FakeSignatureProvider) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            document_signed: Arc::new(Mutex::new(false)),
            reconciled_bytes: Arc::new(Mutex::new(Vec::new())),
            calls: Arc::new(Mutex::new(Vec::new())),
            provider: Arc::new(provider),
        }
    }

    /// Seed a request directly, the way `luxesign_request` would hold it after `send`.
    pub fn with_request(
        self,
        id: &str,
        transaction_document_id: &str,
        status: SignatureRequestStatus,
    ) -> Self {
        self.requests
            .lock()
            .expect("the store is never poisoned")
            .insert(
                id.to_owned(),
                StoredRequest {
                    request: SignatureRequest {
                        id: id.to_owned(),
                        transaction_document_id: transaction_document_id.to_owned(),
                        status,
                        message: None,
                        execution_role: None,
                        execution_slot_id: None,
                        created_by_user_id: None,
                        created_at: "2026-10-07T00:00:00Z".into(),
                        updated_at: "2026-10-07T00:00:00Z".into(),
                    },
                },
            );
        self
    }

    /// Every mutating store call, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .expect("the store is never poisoned")
            .clone()
    }

    /// The signed-artifact payloads the store was asked to reconcile, in order.
    pub fn reconciled_bytes(&self) -> Vec<Vec<u8>> {
        self.reconciled_bytes
            .lock()
            .expect("the store is never poisoned")
            .clone()
    }

    /// Whether the store's reconcile marked the document signed.
    pub fn document_signed(&self) -> bool {
        *self
            .document_signed
            .lock()
            .expect("the store is never poisoned")
    }

    /// The status the store currently holds for `id`.
    pub fn status(&self, id: &str) -> Option<SignatureRequestStatus> {
        self.requests
            .lock()
            .expect("the store is never poisoned")
            .get(id)
            .map(|stored| stored.request.status)
    }

    fn record(&self, call: &str) {
        self.calls
            .lock()
            .expect("the store is never poisoned")
            .push(call.to_owned());
    }

    /// The real service over this store, with the configured provider.
    pub fn service(&self) -> SignatureService<SignatureHarness> {
        SignatureService::new(self.clone(), self.provider.clone(), infrastructure())
    }
}

#[async_trait]
impl SignatureRepository for SignatureHarness {
    async fn get(&self, id: &str) -> DbResult<Option<SignatureRequest>> {
        Ok(self
            .requests
            .lock()
            .expect("the store is never poisoned")
            .get(id)
            .map(|stored| stored.request.clone()))
    }

    async fn active_for_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Option<SignatureRequest>> {
        Ok(self
            .requests
            .lock()
            .expect("the store is never poisoned")
            .values()
            .find(|stored| {
                stored.request.transaction_document_id == transaction_document_id
                    && stored.request.status.is_active()
            })
            .map(|stored| stored.request.clone()))
    }

    async fn list_by_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Vec<SignatureRequest>> {
        Ok(self
            .requests
            .lock()
            .expect("the store is never poisoned")
            .values()
            .filter(|stored| stored.request.transaction_document_id == transaction_document_id)
            .map(|stored| stored.request.clone())
            .collect())
    }

    async fn send(
        &self,
        request: &SendSignatureRequest,
        _actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.record("send");
        let existing = self
            .requests
            .lock()
            .expect("the store is never poisoned")
            .values()
            .any(|stored| {
                stored.request.transaction_document_id == request.transaction_document_id
                    && stored.request.status.is_active()
            });
        if existing {
            return Ok(SignatureCommandResult {
                command_id: request.command_id.clone(),
                outcome: SignatureCommandOutcome::Conflict,
                aggregate_id: Some(request.transaction_document_id.clone()),
                message: Some("An active signature request already exists.".into()),
                replayed: false,
                value: None,
            });
        }
        let stored = SignatureRequest {
            id: format!("sig-{}", self.requests.lock().unwrap().len() + 1),
            transaction_document_id: request.transaction_document_id.clone(),
            status: SignatureRequestStatus::Requested,
            message: request.message.clone(),
            execution_role: request.execution_role.clone(),
            execution_slot_id: request.execution_slot_id.clone(),
            created_by_user_id: request.created_by_user_id.clone(),
            created_at: "2026-10-07T00:00:00Z".into(),
            updated_at: "2026-10-07T00:00:00Z".into(),
        };
        let value = SignatureRequestResult {
            existing: false,
            luxesign_request: stored.clone(),
        };
        self.requests
            .lock()
            .expect("the store is never poisoned")
            .insert(stored.id.clone(), StoredRequest { request: stored });
        Ok(SignatureCommandResult {
            command_id: request.command_id.clone(),
            outcome: SignatureCommandOutcome::Success,
            aggregate_id: Some(value.luxesign_request.id.clone()),
            message: None,
            replayed: false,
            value: Some(serde_json::to_value(value).map_err(|error| {
                DbFailure::schema_mismatch("signature.send.serialize", error.to_string())
            })?),
        })
    }

    async fn apply_status(
        &self,
        request: &ApplySignatureStatusRequest,
        _actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.record("apply_status");
        let mut requests = self.requests.lock().expect("the store is never poisoned");
        let Some(stored) = requests.get_mut(&request.signature_request_id) else {
            return Ok(SignatureCommandResult {
                command_id: request.command_id.clone(),
                outcome: SignatureCommandOutcome::NotFound,
                aggregate_id: None,
                message: Some("Signature request not found.".into()),
                replayed: false,
                value: None,
            });
        };
        let current = stored.request.status;
        let (transitioned, target) = match request.target_status {
            None => (false, current),
            Some(target) if target == current => (false, current),
            Some(target) if !current.can_transition_to(target) => {
                return Ok(SignatureCommandResult {
                    command_id: request.command_id.clone(),
                    outcome: SignatureCommandOutcome::ValidationFailure,
                    aggregate_id: Some(stored.request.id.clone()),
                    message: Some(format!(
                        "Transition {} -> {} is not allowed.",
                        current.as_str(),
                        target.as_str()
                    )),
                    replayed: false,
                    value: None,
                });
            }
            Some(target) => {
                stored.request.status = target;
                (true, target)
            }
        };
        let value = SignatureStatusResult {
            luxesign_request: stored.request.clone(),
            transitioned,
        };
        debug_assert_eq!(value.luxesign_request.status, target);
        Ok(SignatureCommandResult {
            command_id: request.command_id.clone(),
            outcome: SignatureCommandOutcome::Success,
            aggregate_id: Some(stored.request.id.clone()),
            message: None,
            replayed: false,
            value: Some(serde_json::to_value(value).map_err(|error| {
                DbFailure::schema_mismatch("signature.status.serialize", error.to_string())
            })?),
        })
    }

    async fn cancel(
        &self,
        command_id: &str,
        signature_request_id: &str,
        _actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.record("cancel");
        let mut requests = self.requests.lock().expect("the store is never poisoned");
        let Some(stored) = requests.get_mut(signature_request_id) else {
            return Ok(SignatureCommandResult {
                command_id: command_id.to_owned(),
                outcome: SignatureCommandOutcome::NotFound,
                aggregate_id: None,
                message: Some("Signature request not found.".into()),
                replayed: false,
                value: None,
            });
        };
        if !stored
            .request
            .status
            .can_transition_to(SignatureRequestStatus::Voided)
        {
            return Ok(SignatureCommandResult {
                command_id: command_id.to_owned(),
                outcome: SignatureCommandOutcome::Conflict,
                aggregate_id: Some(stored.request.id.clone()),
                message: Some(format!(
                    "Cannot cancel a request in {} state.",
                    stored.request.status.as_str()
                )),
                replayed: false,
                value: None,
            });
        }
        stored.request.status = SignatureRequestStatus::Voided;
        Ok(SignatureCommandResult {
            command_id: command_id.to_owned(),
            outcome: SignatureCommandOutcome::Success,
            aggregate_id: Some(stored.request.id.clone()),
            message: None,
            replayed: false,
            value: None,
        })
    }

    async fn decline(
        &self,
        command_id: &str,
        signature_request_id: &str,
        _actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.record("decline");
        let mut requests = self.requests.lock().expect("the store is never poisoned");
        let Some(stored) = requests.get_mut(signature_request_id) else {
            return Ok(SignatureCommandResult {
                command_id: command_id.to_owned(),
                outcome: SignatureCommandOutcome::NotFound,
                aggregate_id: None,
                message: Some("Signature request not found.".into()),
                replayed: false,
                value: None,
            });
        };
        if !stored
            .request
            .status
            .can_transition_to(SignatureRequestStatus::Declined)
        {
            return Ok(SignatureCommandResult {
                command_id: command_id.to_owned(),
                outcome: SignatureCommandOutcome::Conflict,
                aggregate_id: Some(stored.request.id.clone()),
                message: Some("Cannot decline this request.".into()),
                replayed: false,
                value: None,
            });
        }
        stored.request.status = SignatureRequestStatus::Declined;
        Ok(SignatureCommandResult {
            command_id: command_id.to_owned(),
            outcome: SignatureCommandOutcome::Success,
            aggregate_id: Some(stored.request.id.clone()),
            message: None,
            replayed: false,
            value: None,
        })
    }

    async fn reconciliation_needs_artifact(
        &self,
        _event_id: &str,
        _signature_request_id: &str,
    ) -> DbResult<bool> {
        Ok(!self.document_signed())
    }

    async fn reconcile_completed(
        &self,
        _event_id: &str,
        signature_request_id: &str,
        signed_artifact: Option<&SignatureArtifactDownload>,
        _audit_artifact: Option<&SignatureArtifactDownload>,
        _actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.record("reconcile_completed");
        if self.document_signed() {
            return Ok(SignatureCommandResult {
                command_id: format!("signature.reconcile:{}", _event_id),
                outcome: SignatureCommandOutcome::Success,
                aggregate_id: Some(signature_request_id.to_owned()),
                message: Some("Document already signed; completion treated as replayed.".into()),
                replayed: true,
                value: Some(json!({ "replayed": true })),
            });
        }
        let Some(artifact) = signed_artifact else {
            return Ok(SignatureCommandResult {
                command_id: format!("signature.reconcile:{}", _event_id),
                outcome: SignatureCommandOutcome::Conflict,
                aggregate_id: Some(signature_request_id.to_owned()),
                message: Some(
                    "Signed artifact is required for an unreconciled completed request.".into(),
                ),
                replayed: false,
                value: None,
            });
        };
        self.reconciled_bytes
            .lock()
            .expect("the store is never poisoned")
            .push(artifact.bytes.clone());
        *self
            .document_signed
            .lock()
            .expect("the store is never poisoned") = true;
        Ok(SignatureCommandResult {
            command_id: format!("signature.reconcile:{}", _event_id),
            outcome: SignatureCommandOutcome::Success,
            aggregate_id: Some(signature_request_id.to_owned()),
            message: None,
            replayed: false,
            value: Some(json!({ "replayed": false })),
        })
    }
}
