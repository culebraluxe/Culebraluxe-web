//! Port of `workflow_app/forge/forge-facts.ts`.
//! Booleans default FALSE. Enum routers omitted when absent (fail closed).

use workflow::Value;

use crate::engine::qa_repair::{
    route_qa_result, QaDisposition, QaVerdict, RepairAttemptState, RepairBudget,
};

#[derive(Debug, Clone, Default)]
pub struct ForgeGateEvidence {
    pub work_type: Option<String>,
    pub research_disposition: Option<String>,
    pub scout_required: Option<bool>,
    pub root_cause_known: Option<bool>,
    pub diagnosis_blocked: Option<bool>,
    pub architecture_suspect: Option<bool>,
    pub architecture_review_required: Option<bool>,
    pub lead_decision: Option<String>,
    pub split_count: Option<i64>,
    pub lead_routing: Option<Value>,
    pub deployment_deferred_to_batch: Option<i64>,
    pub findings: Option<Value>,
    pub deliverable_rejection: Option<String>,
    pub qa_review_required: Option<bool>,
    pub qa_review_passed: Option<bool>,
    pub qa_passed: Option<bool>,
    pub disposition: Option<String>,
    pub verification_gap: Option<bool>,
    pub no_progress: Option<bool>,
    pub repair_attempts: Option<u32>,
    pub replan_attempts: Option<u32>,
    pub last_failure: Option<String>,
    pub failure_class: Option<String>,
    pub failed_release_stage: Option<String>,
    pub classifier_failure_class: Option<String>,
    pub stage_failure_class: Option<String>,
    pub publish_succeeded: Option<bool>,
    pub migration_required: Option<bool>,
    pub migration_files: Option<Vec<String>>,
    pub dev_migration_applied: Option<bool>,
    pub dev_migration_verified: Option<bool>,
    pub prod_migration_applied: Option<bool>,
    pub prod_migration_verified: Option<bool>,
    pub derived_refresh_required: Option<bool>,
    pub derived_models: Option<Vec<String>>,
    pub derived_refresh_succeeded: Option<bool>,
    pub derived_refresh_verified: Option<bool>,
    pub deployment_required: Option<bool>,
    pub deployment_succeeded: Option<bool>,
    pub deployment_deferred: Option<bool>,
    pub release_deferred: Option<bool>,
    pub deployment_receipt: Option<String>,
    pub production_verified: Option<bool>,
    pub production_verification_receipt: Option<String>,
    pub candidate_sha: Option<String>,
    pub qa_verified_sha: Option<String>,
    pub published_sha: Option<String>,
    pub deployed_sha: Option<String>,
    pub production_verified_sha: Option<String>,
    pub batch_released_sha: Option<String>,
    pub batch_released_at: Option<String>,
    pub batch_release_receipt: Option<String>,
    pub resume_target: Option<String>,
    pub extra: Value,
}

fn or_opt<T: Clone>(a: &Option<T>, b: &Option<T>) -> Option<T> {
    a.clone().or_else(|| b.clone())
}

impl ForgeGateEvidence {
    pub fn merge_over(&self, base: &ForgeGateEvidence) -> ForgeGateEvidence {
        let mut extra = base.extra.clone();
        workflow::value::merge(&mut extra, &self.extra);
        ForgeGateEvidence {
            work_type: or_opt(&self.work_type, &base.work_type),
            research_disposition: or_opt(&self.research_disposition, &base.research_disposition),
            scout_required: self.scout_required.or(base.scout_required),
            root_cause_known: self.root_cause_known.or(base.root_cause_known),
            diagnosis_blocked: self.diagnosis_blocked.or(base.diagnosis_blocked),
            architecture_suspect: self.architecture_suspect.or(base.architecture_suspect),
            architecture_review_required: self
                .architecture_review_required
                .or(base.architecture_review_required),
            lead_decision: or_opt(&self.lead_decision, &base.lead_decision),
            split_count: self.split_count.or(base.split_count),
            lead_routing: or_opt(&self.lead_routing, &base.lead_routing),
            deployment_deferred_to_batch: self
                .deployment_deferred_to_batch
                .or(base.deployment_deferred_to_batch),
            findings: or_opt(&self.findings, &base.findings),
            deliverable_rejection: or_opt(&self.deliverable_rejection, &base.deliverable_rejection),
            qa_review_required: self.qa_review_required.or(base.qa_review_required),
            qa_review_passed: self.qa_review_passed.or(base.qa_review_passed),
            qa_passed: self.qa_passed.or(base.qa_passed),
            disposition: or_opt(&self.disposition, &base.disposition),
            verification_gap: self.verification_gap.or(base.verification_gap),
            no_progress: self.no_progress.or(base.no_progress),
            repair_attempts: self.repair_attempts.or(base.repair_attempts),
            replan_attempts: self.replan_attempts.or(base.replan_attempts),
            last_failure: or_opt(&self.last_failure, &base.last_failure),
            failure_class: or_opt(&self.failure_class, &base.failure_class),
            failed_release_stage: or_opt(&self.failed_release_stage, &base.failed_release_stage),
            classifier_failure_class: or_opt(
                &self.classifier_failure_class,
                &base.classifier_failure_class,
            ),
            stage_failure_class: or_opt(&self.stage_failure_class, &base.stage_failure_class),
            publish_succeeded: self.publish_succeeded.or(base.publish_succeeded),
            migration_required: self.migration_required.or(base.migration_required),
            migration_files: or_opt(&self.migration_files, &base.migration_files),
            dev_migration_applied: self.dev_migration_applied.or(base.dev_migration_applied),
            dev_migration_verified: self.dev_migration_verified.or(base.dev_migration_verified),
            prod_migration_applied: self.prod_migration_applied.or(base.prod_migration_applied),
            prod_migration_verified: self
                .prod_migration_verified
                .or(base.prod_migration_verified),
            derived_refresh_required: self
                .derived_refresh_required
                .or(base.derived_refresh_required),
            derived_models: or_opt(&self.derived_models, &base.derived_models),
            derived_refresh_succeeded: self
                .derived_refresh_succeeded
                .or(base.derived_refresh_succeeded),
            derived_refresh_verified: self
                .derived_refresh_verified
                .or(base.derived_refresh_verified),
            deployment_required: self.deployment_required.or(base.deployment_required),
            deployment_succeeded: self.deployment_succeeded.or(base.deployment_succeeded),
            deployment_deferred: self.deployment_deferred.or(base.deployment_deferred),
            release_deferred: self.release_deferred.or(base.release_deferred),
            deployment_receipt: or_opt(&self.deployment_receipt, &base.deployment_receipt),
            production_verified: self.production_verified.or(base.production_verified),
            production_verification_receipt: or_opt(
                &self.production_verification_receipt,
                &base.production_verification_receipt,
            ),
            candidate_sha: or_opt(&self.candidate_sha, &base.candidate_sha),
            qa_verified_sha: or_opt(&self.qa_verified_sha, &base.qa_verified_sha),
            published_sha: or_opt(&self.published_sha, &base.published_sha),
            deployed_sha: or_opt(&self.deployed_sha, &base.deployed_sha),
            production_verified_sha: or_opt(
                &self.production_verified_sha,
                &base.production_verified_sha,
            ),
            batch_released_sha: or_opt(&self.batch_released_sha, &base.batch_released_sha),
            batch_released_at: or_opt(&self.batch_released_at, &base.batch_released_at),
            batch_release_receipt: or_opt(&self.batch_release_receipt, &base.batch_release_receipt),
            resume_target: or_opt(&self.resume_target, &base.resume_target),
            extra,
        }
    }

    pub fn to_facts(&self) -> Value {
        project_forge_gate_facts(self)
    }
}

fn normalized_sha(value: Option<&str>) -> Option<String> {
    let sha = value.unwrap_or("").trim().to_ascii_lowercase();
    if sha.len() >= 7 && sha.len() <= 64 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(sha)
    } else {
        None
    }
}

pub fn forge_lineage_error(evidence: &ForgeGateEvidence, stage: &str) -> Option<String> {
    if normalized_sha(evidence.candidate_sha.as_deref()).is_none() {
        return Some("candidateSha is missing or invalid".into());
    }
    if evidence.qa_passed != Some(true) {
        return Some("QA has not passed for this candidate".into());
    }
    if stage == "qa" {
        return None;
    }
    if normalized_sha(evidence.published_sha.as_deref()).is_none() {
        return Some("publishedSha is missing or invalid".into());
    }
    if stage == "publish" {
        return None;
    }
    if stage == "deploy" {
        let deployed = match normalized_sha(evidence.deployed_sha.as_deref()) {
            Some(s) => s,
            None => return Some("deployedSha is missing or invalid".into()),
        };
        let published = normalized_sha(evidence.published_sha.as_deref()).unwrap();
        if deployed != published {
            return Some(format!(
                "deployed {deployed}, expected published {published}"
            ));
        }
        return None;
    }
    let expected = if evidence.deployment_required == Some(true) {
        normalized_sha(evidence.deployed_sha.as_deref())
    } else {
        normalized_sha(evidence.published_sha.as_deref())
    };
    let expected = match expected {
        Some(s) => s,
        None => return Some("production artifact source SHA is missing or invalid".into()),
    };
    let verified = match normalized_sha(evidence.production_verified_sha.as_deref()) {
        Some(s) => s,
        None => return Some("productionVerifiedSha is missing or invalid".into()),
    };
    if verified != expected {
        return Some(format!(
            "production verified {verified}, expected {expected}"
        ));
    }
    None
}

pub fn forge_deployment_producer_configured(evidence: &ForgeGateEvidence) -> bool {
    (normalized_sha(evidence.deployed_sha.as_deref()).is_some()
        && evidence
            .deployment_receipt
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false))
        || (normalized_sha(evidence.production_verified_sha.as_deref()).is_some()
            && evidence
                .production_verification_receipt
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false))
}

pub fn forge_deploy_hold_reason(evidence: &ForgeGateEvidence) -> Option<&'static str> {
    if evidence.deployment_required != Some(true) {
        return None;
    }
    if evidence.deployment_deferred_to_batch.is_some() {
        return None;
    }
    if forge_deployment_producer_configured(evidence) {
        return None;
    }
    Some("no deployment producer is configured")
}

pub fn forge_fast_eligibility(evidence: &ForgeGateEvidence) -> bool {
    evidence.work_type.as_deref() == Some("FAST")
        && evidence.migration_required != Some(true)
        && evidence.derived_refresh_required != Some(true)
        && evidence.deployment_required != Some(true)
        && evidence.architecture_suspect != Some(true)
        && evidence.lead_decision.as_deref() != Some("SPLIT")
}

pub fn project_forge_gate_facts(evidence: &ForgeGateEvidence) -> Value {
    let mut facts = evidence.extra.clone();
    let enums = [
        ("workType", evidence.work_type.as_deref()),
        (
            "researchDisposition",
            evidence.research_disposition.as_deref(),
        ),
        ("leadDecision", evidence.lead_decision.as_deref()),
        ("disposition", evidence.disposition.as_deref()),
        ("failureClass", evidence.failure_class.as_deref()),
        (
            "failedReleaseStage",
            evidence.failed_release_stage.as_deref(),
        ),
        ("resumeTarget", evidence.resume_target.as_deref()),
    ];
    for (k, v) in enums {
        if let Some(v) = v {
            facts.insert(k, Value::from(v));
        }
    }
    if let Some(n) = evidence.split_count {
        facts.insert("splitCount", Value::from(n));
    }
    if let Some(n) = evidence.repair_attempts {
        facts.insert("repairAttempts", Value::from(n as i64));
    }
    if let Some(n) = evidence.replan_attempts {
        facts.insert("replanAttempts", Value::from(n as i64));
    }

    let disp = match evidence.disposition.as_deref() {
        Some("REPAIR") => Some(QaDisposition::Repair),
        Some("REPLAN") => Some(QaDisposition::Replan),
        Some("ESCALATE") => Some(QaDisposition::Escalate),
        _ => None,
    };
    let qa_route = if evidence.qa_passed == Some(false) {
        Some(route_qa_result(
            QaVerdict::Fail,
            disp,
            RepairAttemptState {
                repair_attempts: evidence.repair_attempts.unwrap_or(0),
                replan_attempts: evidence.replan_attempts.unwrap_or(0),
            },
            RepairBudget::default(),
            evidence.no_progress == Some(true),
            evidence.verification_gap == Some(true),
        ))
    } else {
        None
    };

    let repair_el = matches!(
        qa_route,
        Some(crate::engine::qa_repair::RepairRouting::Smith { .. })
    );
    let replan_el = matches!(
        qa_route,
        Some(crate::engine::qa_repair::RepairRouting::Architect { .. })
    );
    let pairs: Vec<(&str, bool)> = vec![
        ("scoutRequired", evidence.scout_required.unwrap_or(false)),
        ("rootCauseKnown", evidence.root_cause_known.unwrap_or(false)),
        (
            "diagnosisBlocked",
            evidence.diagnosis_blocked.unwrap_or(false),
        ),
        (
            "architectureSuspect",
            evidence.architecture_suspect.unwrap_or(false),
        ),
        (
            "architectureReviewRequired",
            evidence.architecture_review_required.unwrap_or(false),
        ),
        (
            "qaReviewRequired",
            evidence.qa_review_required.unwrap_or(false),
        ),
        ("qaReviewPassed", evidence.qa_review_passed.unwrap_or(false)),
        ("qaPassed", evidence.qa_passed == Some(true)),
        ("qaRepairEligible", repair_el),
        ("qaReplanEligible", replan_el),
        (
            "publishSucceeded",
            evidence.publish_succeeded == Some(true)
                && forge_lineage_error(evidence, "publish").is_none(),
        ),
        (
            "migrationRequired",
            evidence.migration_required.unwrap_or(false),
        ),
        (
            "devMigrationApplied",
            evidence.dev_migration_applied.unwrap_or(false),
        ),
        (
            "devMigrationVerified",
            evidence.dev_migration_verified.unwrap_or(false),
        ),
        (
            "prodMigrationApplied",
            evidence.prod_migration_applied.unwrap_or(false),
        ),
        (
            "prodMigrationVerified",
            evidence.prod_migration_verified.unwrap_or(false),
        ),
        (
            "derivedRefreshRequired",
            evidence.derived_refresh_required.unwrap_or(false),
        ),
        (
            "derivedRefreshSucceeded",
            evidence.derived_refresh_succeeded.unwrap_or(false),
        ),
        (
            "derivedRefreshVerified",
            evidence.derived_refresh_verified.unwrap_or(false),
        ),
        (
            "deploymentRequired",
            evidence.deployment_required.unwrap_or(false),
        ),
        (
            "deploymentDeferred",
            evidence.deployment_deferred_to_batch.is_some(),
        ),
        (
            "releaseDeferred",
            evidence.deployment_deferred_to_batch.is_some(),
        ),
        (
            "deploymentSucceeded",
            evidence.deployment_succeeded == Some(true)
                && forge_lineage_error(evidence, "deploy").is_none(),
        ),
        (
            "productionVerified",
            evidence.production_verified == Some(true)
                && forge_lineage_error(evidence, "production").is_none(),
        ),
        (
            "deploymentProducerConfigured",
            forge_deployment_producer_configured(evidence),
        ),
        (
            "deploymentBlocked",
            forge_deploy_hold_reason(evidence).is_some(),
        ),
    ];
    for (k, v) in pairs {
        facts.insert(k, Value::from(v));
    }
    facts.insert(
        "fastEligible",
        Value::from(forge_fast_eligibility(evidence)),
    );
    if let Some(s) = &evidence.candidate_sha {
        facts.insert("candidateSha", Value::from(s.as_str()));
    }
    if let Some(s) = &evidence.qa_verified_sha {
        facts.insert("qaVerifiedSha", Value::from(s.as_str()));
    }
    if let Some(s) = &evidence.published_sha {
        facts.insert("publishedSha", Value::from(s.as_str()));
    }
    facts
}

pub fn project_gate_facts(evidence: &ForgeGateEvidence) -> Value {
    project_forge_gate_facts(evidence)
}

pub fn evidence_from_value(v: &Value) -> ForgeGateEvidence {
    let get_s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());
    let get_b = |k: &str| match v.get(k) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    };
    let get_i = |k: &str| match v.get(k) {
        Some(Value::Number(n)) => Some(*n as i64),
        _ => None,
    };
    ForgeGateEvidence {
        work_type: get_s("workType"),
        research_disposition: get_s("researchDisposition"),
        scout_required: get_b("scoutRequired"),
        root_cause_known: get_b("rootCauseKnown"),
        diagnosis_blocked: get_b("diagnosisBlocked"),
        architecture_suspect: get_b("architectureSuspect"),
        architecture_review_required: get_b("architectureReviewRequired"),
        lead_decision: get_s("leadDecision"),
        split_count: get_i("splitCount"),
        qa_passed: get_b("qaPassed"),
        disposition: get_s("disposition"),
        verification_gap: get_b("verificationGap"),
        no_progress: get_b("noProgress"),
        failure_class: get_s("failureClass"),
        failed_release_stage: get_s("failedReleaseStage"),
        publish_succeeded: get_b("publishSucceeded"),
        candidate_sha: get_s("candidateSha"),
        qa_verified_sha: get_s("qaVerifiedSha"),
        published_sha: get_s("publishedSha"),
        deployed_sha: get_s("deployedSha"),
        resume_target: get_s("resumeTarget"),
        extra: v.clone(),
        ..Default::default()
    }
}
