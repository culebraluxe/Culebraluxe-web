use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, RelationshipEvidenceDao};
use domain::{
    RelationshipDecision, RelationshipEvidenceReview, RelationshipEvidenceRow,
    RelationshipReconcileResult,
};
use serde_json::Value;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::{BTreeMap, HashSet};

pub const REL_INTEL_RULE_VERSION: &str = "rel-intel/v1";

#[async_trait]
pub trait RelationshipEvidenceRepository: Send {
    async fn review(
        &mut self,
        review_state: &str,
        search: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<RelationshipEvidenceReview>;
    async fn by_id(&mut self, id: &str) -> DbResult<Option<RelationshipEvidenceRow>>;
    async fn classify(
        &mut self,
        id: &str,
        automated: Option<bool>,
        service: Option<bool>,
    ) -> DbResult<bool>;
    async fn person_exists(&mut self, person_id: &str) -> DbResult<bool>;
    async fn source_link(
        &mut self,
        source: &str,
        source_account: &str,
        source_identity_key: &str,
    ) -> DbResult<Option<String>>;
    async fn people_by_email(&mut self, email: &str) -> DbResult<Vec<String>>;
    async fn people_by_phone(&mut self, phone: &str) -> DbResult<Vec<String>>;
    async fn candidates(
        &mut self,
        source: Option<&str>,
        review_state: Option<&str>,
        ids: &[String],
        limit: i64,
    ) -> DbResult<Vec<RelationshipEvidenceRow>>;
    async fn record_decision(
        &mut self,
        id: &str,
        decision: &RelationshipDecision,
    ) -> DbResult<bool>;
}

#[async_trait]
impl RelationshipEvidenceRepository for RelationshipEvidenceDao {
    async fn review(
        &mut self,
        review_state: &str,
        search: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<RelationshipEvidenceReview> {
        RelationshipEvidenceDao::review(self, review_state, search, limit, offset).await
    }

    async fn by_id(&mut self, id: &str) -> DbResult<Option<RelationshipEvidenceRow>> {
        RelationshipEvidenceDao::by_id(self, id).await
    }

    async fn classify(
        &mut self,
        id: &str,
        automated: Option<bool>,
        service: Option<bool>,
    ) -> DbResult<bool> {
        RelationshipEvidenceDao::classify(self, id, automated, service).await
    }

    async fn person_exists(&mut self, person_id: &str) -> DbResult<bool> {
        RelationshipEvidenceDao::person_exists(self, person_id).await
    }

    async fn source_link(
        &mut self,
        source: &str,
        source_account: &str,
        source_identity_key: &str,
    ) -> DbResult<Option<String>> {
        RelationshipEvidenceDao::source_link(self, source, source_account, source_identity_key)
            .await
    }

    async fn people_by_email(&mut self, email: &str) -> DbResult<Vec<String>> {
        RelationshipEvidenceDao::people_by_email(self, email).await
    }

    async fn people_by_phone(&mut self, phone: &str) -> DbResult<Vec<String>> {
        RelationshipEvidenceDao::people_by_phone(self, phone).await
    }

    async fn candidates(
        &mut self,
        source: Option<&str>,
        review_state: Option<&str>,
        ids: &[String],
        limit: i64,
    ) -> DbResult<Vec<RelationshipEvidenceRow>> {
        RelationshipEvidenceDao::candidates(self, source, review_state, ids, limit).await
    }

    async fn record_decision(
        &mut self,
        id: &str,
        decision: &RelationshipDecision,
    ) -> DbResult<bool> {
        RelationshipEvidenceDao::record_decision(self, id, decision).await
    }
}

pub struct RelationshipEvidenceService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: RelationshipEvidenceRepository> RelationshipEvidenceService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn review(
        &mut self,
        review_state: &str,
        search: &str,
        limit: i64,
        offset: i64,
        context: &ServiceContext,
    ) -> Result<RelationshipEvidenceReview, CoreServiceError> {
        const OP: &str = "relationshipEvidence.review";
        let decision = authorize(
            &self.runtime,
            "relationship",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .review(review_state, search, limit.clamp(1, 100), offset.max(0))
            .await
            .map_err(Into::into);
        audit_result(
            &self.runtime,
            "relationship",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn inspect(
        &mut self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<RelationshipEvidenceRow>, CoreServiceError> {
        const OP: &str = "relationshipEvidence.inspect";
        let decision = authorize(
            &self.runtime,
            "relationship",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.by_id(id).await.map_err(Into::into);
        audit_result(
            &self.runtime,
            "relationship",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn classify_and_rerun(
        &mut self,
        id: &str,
        automated: bool,
        service_flag: bool,
        context: &ServiceContext,
    ) -> Result<RelationshipReconcileResult, CoreServiceError> {
        const OP: &str = "relationshipEvidence.classify";
        let decision = authorize(
            &self.runtime,
            "relationship",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let updated = self
                .repository
                .classify(id, automated.then_some(true), service_flag.then_some(true))
                .await?;
            if !updated {
                return Err(CoreServiceError::business(
                    "RELATIONSHIP_EVIDENCE_NOT_FOUND",
                    "Evidence row not found.",
                ));
            }
            self.rerun_authorized(None, None, &[id.to_owned()], 1).await
        }
        .await;
        audit_result(
            &self.runtime,
            "relationship",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn link(
        &mut self,
        id: &str,
        person_id: &str,
        confirmed: bool,
        context: &ServiceContext,
    ) -> Result<Option<RelationshipEvidenceRow>, CoreServiceError> {
        const OP: &str = "relationshipEvidence.link";
        let decision = authorize(
            &self.runtime,
            "relationship",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            if !confirmed {
                return Err(CoreServiceError::business(
                    "RELATIONSHIP_CONFIRMATION_REQUIRED",
                    "Explicit confirmation required.",
                ));
            }
            if !self.repository.person_exists(person_id).await? {
                return Err(CoreServiceError::business(
                    "PERSON_NOT_FOUND",
                    "Person not found.",
                ));
            }
            let write = RelationshipDecision {
                review_state: "exact_linked".into(),
                match_method: "source_link".into(),
                match_confidence: "exact".into(),
                canonical_person_id: Some(person_id.to_owned()),
                reason: "opps_operator_approval".into(),
                rule_version: REL_INTEL_RULE_VERSION.into(),
            };
            if !self.repository.record_decision(id, &write).await? {
                return Err(CoreServiceError::business(
                    "RELATIONSHIP_EVIDENCE_NOT_FOUND",
                    "Evidence row not found.",
                ));
            }
            self.repository.by_id(id).await.map_err(Into::into)
        }
        .await;
        audit_result(
            &self.runtime,
            "relationship",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn reject(
        &mut self,
        id: &str,
        confirmed: bool,
        context: &ServiceContext,
    ) -> Result<Option<RelationshipEvidenceRow>, CoreServiceError> {
        const OP: &str = "relationshipEvidence.reject";
        let decision = authorize(
            &self.runtime,
            "relationship",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            if !confirmed {
                return Err(CoreServiceError::business(
                    "RELATIONSHIP_CONFIRMATION_REQUIRED",
                    "Explicit confirmation required.",
                ));
            }
            let write = RelationshipDecision {
                review_state: "rejected".into(),
                match_method: "rejected".into(),
                match_confidence: "none".into(),
                canonical_person_id: None,
                reason: "opps_operator_dismissal".into(),
                rule_version: REL_INTEL_RULE_VERSION.into(),
            };
            if !self.repository.record_decision(id, &write).await? {
                return Err(CoreServiceError::business(
                    "RELATIONSHIP_EVIDENCE_NOT_FOUND",
                    "Evidence row not found.",
                ));
            }
            self.repository.by_id(id).await.map_err(Into::into)
        }
        .await;
        audit_result(
            &self.runtime,
            "relationship",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    pub async fn rerun(
        &mut self,
        source: Option<&str>,
        review_state: Option<&str>,
        limit: i64,
        context: &ServiceContext,
    ) -> Result<RelationshipReconcileResult, CoreServiceError> {
        const OP: &str = "relationshipEvidence.rerun";
        let decision = authorize(
            &self.runtime,
            "relationship",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self
            .rerun_authorized(source, review_state, &[], limit.clamp(1, 500))
            .await;
        audit_result(
            &self.runtime,
            "relationship",
            OP,
            context,
            decision,
            &result,
        )
        .await?;
        result
    }

    async fn rerun_authorized(
        &mut self,
        source: Option<&str>,
        review_state: Option<&str>,
        ids: &[String],
        limit: i64,
    ) -> Result<RelationshipReconcileResult, CoreServiceError> {
        let rows = self
            .repository
            .candidates(source, review_state, ids, limit)
            .await?;

        let mut tally = [
            "unresolved",
            "exact_linked",
            "review_required",
            "ambiguous",
            "unmatched",
            "rejected",
            "non_person",
            "deferred",
        ]
        .into_iter()
        .map(|key| (key.to_owned(), 0_i64))
        .collect::<BTreeMap<_, _>>();
        let mut canonical_linked = 0_i64;

        for row in &rows {
            let decision = self.reconcile(row).await?;
            if let Some(value) = tally.get_mut(&decision.review_state) {
                *value += 1;
            }
            if decision.review_state == "exact_linked" && decision.canonical_person_id.is_some() {
                canonical_linked += 1;
            }
            self.repository.record_decision(&row.id, &decision).await?;
        }

        Ok(RelationshipReconcileResult {
            rows,
            tally,
            canonical_linked,
        })
    }

    async fn reconcile(
        &mut self,
        row: &RelationshipEvidenceRow,
    ) -> Result<RelationshipDecision, CoreServiceError> {
        if row.is_organization_or_service.unwrap_or(false) {
            return Ok(decision(
                "non_person",
                "rejected",
                "none",
                "automated_service_or_organization",
                None,
            ));
        }
        if row.is_automated_or_bulk.unwrap_or(false) {
            return Ok(decision(
                "rejected",
                "rejected",
                "none",
                "automated_or_bulk_evidence",
                None,
            ));
        }

        if let Some(person_id) = self
            .repository
            .source_link(&row.source, &row.source_account, &row.source_identity_key)
            .await?
        {
            return Ok(decision(
                "exact_linked",
                "source_link",
                "exact",
                "explicit_source_link",
                Some(person_id),
            ));
        }

        let emails = normalized_values(&row.emails, false);
        let phones = normalized_values(&row.phones, true);
        let mut owners = HashSet::new();
        let mut multi_match = false;
        let mut method: Option<&str> = None;

        for email in emails {
            let people = self.repository.people_by_email(&email).await?;
            if people.len() > 1 {
                multi_match = true;
            } else if let Some(person_id) = people.first() {
                owners.insert(person_id.clone());
                method.get_or_insert("exact_email");
            }
        }
        for phone in phones {
            let people = self.repository.people_by_phone(&phone).await?;
            if people.len() > 1 {
                multi_match = true;
            } else if let Some(person_id) = people.first() {
                owners.insert(person_id.clone());
                method.get_or_insert("exact_phone");
            }
        }

        if multi_match {
            return Ok(decision(
                "ambiguous",
                method.unwrap_or("exact_email"),
                "ambiguous",
                "identity_matches_multiple_people",
                None,
            ));
        }
        if owners.len() == 1 && method.is_some() {
            return Ok(decision(
                "exact_linked",
                method.unwrap_or("exact_email"),
                "exact",
                "exact_normalized_identity",
                owners.into_iter().next(),
            ));
        }
        if owners.len() > 1 {
            return Ok(decision(
                "ambiguous",
                method.unwrap_or("exact_email"),
                "ambiguous",
                "cross_identity_conflict",
                None,
            ));
        }

        if !row.has_email && !row.has_phone {
            return Ok(decision(
                "deferred",
                "unmatched",
                "none",
                "insufficient_identity_evidence",
                None,
            ));
        }
        let meaningful = row.is_two_way.unwrap_or(false)
            || row.is_owner_initiated.unwrap_or(false)
            || row.outbound_count.unwrap_or(0) > 0;
        if meaningful {
            return Ok(decision(
                "review_required",
                "review_candidate",
                "probable",
                "two_way_or_owner_initiated_without_exact_match",
                None,
            ));
        }

        Ok(decision(
            "unmatched",
            "unmatched",
            "none",
            "no_exact_match",
            None,
        ))
    }
}

fn decision(
    review_state: &str,
    match_method: &str,
    match_confidence: &str,
    reason: &str,
    canonical_person_id: Option<String>,
) -> RelationshipDecision {
    RelationshipDecision {
        review_state: review_state.into(),
        match_method: match_method.into(),
        match_confidence: match_confidence.into(),
        canonical_person_id,
        reason: reason.into(),
        rule_version: REL_INTEL_RULE_VERSION.into(),
    }
}

fn normalized_values(value: &Value, phone: bool) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("normalized").and_then(Value::as_str))
        .map(|value| {
            if phone {
                let digits = value
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>();
                if digits.len() == 11 && digits.starts_with('1') {
                    digits[1..].to_owned()
                } else {
                    digits
                }
            } else {
                value.trim().to_ascii_lowercase()
            }
        })
        .filter(|value| !value.is_empty())
        .collect()
}
