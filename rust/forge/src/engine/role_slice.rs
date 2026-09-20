//! First production slice of agent-runtime-role-runner.ts.

use crate::engine::integration::{
    attest_integration, git_is_ancestor, release_evidence_from_integration, BuildObservation,
};
use crate::engine::release_receipt::ReleaseEvidence;
use workflow::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyExistingArrangement {
    pub lane: &'static str,
    pub node: &'static str,
    pub candidate_sha: String,
    pub proofs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LeadProposal {
    pub decision: String,
    pub verify_candidate: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RoutingReview {
    Fail { errors: Vec<String> },
    Ok { proposal: LeadProposal },
}

pub fn assay_route_arrangement(
    review: &RoutingReview,
    proofs: &[String],
) -> Option<VerifyExistingArrangement> {
    let RoutingReview::Ok { proposal } = review else {
        return None;
    };
    if proposal.decision != "ASSAY" {
        return None;
    }
    let sha = proposal
        .verify_candidate
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(VerifyExistingArrangement {
        lane: "assay",
        node: "qa_verify",
        candidate_sha: sha,
        proofs: proofs.to_vec(),
    })
}

pub fn forge_lane_surface(form_data: Option<&Value>) -> Option<Vec<String>> {
    let form = form_data?;
    let clean = |v: &Value| -> Vec<String> {
        match v {
            Value::Array(items) => items
                .iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => vec![],
        }
    };
    let direct = form.get("surface").map(clean).unwrap_or_default();
    if !direct.is_empty() {
        return Some(direct);
    }
    let slice = form.get("splitBranch")?;
    let plan = slice.get("plan")?;
    let chunks = match plan.get("chunks") {
        Some(Value::Array(items)) => items,
        _ => return None,
    };
    let mut from_plan = Vec::new();
    for chunk in chunks {
        if let Some(surface) = chunk.get("surface") {
            from_plan.extend(clean(surface));
        }
    }
    if from_plan.is_empty() {
        None
    } else {
        Some(from_plan)
    }
}

/// Derive a release receipt the same way the TS runner does. Proofs are injected.
pub fn derive_release_evidence(
    node_id: &str,
    deployment_required: bool,
    published_sha: Option<&str>,
    deployment_receipt: Option<&str>,
    deployed_sha: Option<&str>,
    production_verification_receipt: Option<&str>,
    production_verified_sha: Option<&str>,
    proofs: &[BuildObservation],
    cwd: &str,
) -> Option<ReleaseEvidence> {
    if let Some(receipt) = production_verification_receipt.filter(|s| !s.trim().is_empty()) {
        return Some(ReleaseEvidence {
            kind: crate::engine::release_receipt::ReleaseReceiptKind::ProductionVerification,
            artifact_sha: production_verified_sha.unwrap_or("").to_string(),
            receipt_id: receipt.to_string(),
            success: true,
        });
    }
    if let Some(receipt) = deployment_receipt.filter(|s| !s.trim().is_empty()) {
        return Some(ReleaseEvidence {
            kind: crate::engine::release_receipt::ReleaseReceiptKind::Deployment,
            artifact_sha: deployed_sha.unwrap_or("").to_string(),
            receipt_id: receipt.to_string(),
            success: true,
        });
    }
    if node_id != "deploy" || deployment_required {
        return None;
    }
    let attestation = attest_integration(
        published_sha,
        "origin/main",
        |sha, r#ref| Ok(git_is_ancestor(cwd, sha, r#ref)),
        None,
        if proofs.is_empty() {
            None
        } else {
            Some(proofs.to_vec())
        },
    );
    release_evidence_from_integration(&attestation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow::Value;

    #[test]
    fn assay_only_when_sha_is_full() {
        let review = RoutingReview::Ok {
            proposal: LeadProposal {
                decision: "ASSAY".into(),
                verify_candidate: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
            },
        };
        let arr = assay_route_arrangement(&review, &["npm test".into()]).unwrap();
        assert_eq!(arr.node, "qa_verify");
        let short = RoutingReview::Ok {
            proposal: LeadProposal {
                decision: "ASSAY".into(),
                verify_candidate: Some("abc".into()),
            },
        };
        assert!(assay_route_arrangement(&short, &[]).is_none());
    }

    #[test]
    fn surface_from_form() {
        use std::collections::BTreeMap;
        let mut form = BTreeMap::new();
        form.insert(
            "surface".into(),
            Value::Array(vec![
                Value::String("src/a.rs".into()),
                Value::String("src/b.rs".into()),
            ]),
        );
        let surface = forge_lane_surface(Some(&Value::Object(form))).unwrap();
        assert_eq!(surface, vec!["src/a.rs", "src/b.rs"]);
    }
}
