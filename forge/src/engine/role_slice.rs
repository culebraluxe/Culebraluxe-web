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

/// The bench intents a dispatch may carry (migration 167 `launch_intent`; the CHECK that admits them lives on the
/// column). The Cockpit sets one when the operator caps the Lead; NULL means "the Lead decides".
pub const BENCH_INTENTS: [&str; 4] = ["SOLO", "SMITH", "SPLIT", "HOLD"];

/// Does the Lead's decision honour the bench intent the dispatch carried? Empty = it does, or there was no cap.
///
/// Ported from `benchIntentErrors` (`legacy/workflow_app/forge/forge-lead-routing.ts`, deleted with `legacy/`). The
/// assertions that outlive it are the contract:
/// `legacy/workflow_app/forge/agents/handoff.test.ts:29-33` — `benchIntentErrors('SMITH','HOLD')` errors,
/// `('HOLD','HOLD')` is clean, `('SPLIT','SOLO')` errors — and
/// `legacy/workflow_app/tests/forge-lead-routing-bench.test.ts:88-140` — a `HOLD` bench refuses any decision that is
/// not `HOLD` ("Bench intent is HOLD"), and a `SOLO` bench refuses `SPLIT` ("Bench intent is SOLO").
///
/// The rule is equality in both directions: the intent is a CAP, so a decision broader than the cap (SPLIT under a
/// SOLO bench) and a decision narrower than it (SOLO under a SMITH bench) are both the Lead substituting its own
/// judgement for the operator's. A null or blank intent is not a cap and decides nothing here, and a lane with no
/// decision yet is left to the missing-decision rail rather than refused twice.
pub fn bench_intent_errors(intent: Option<&str>, decision: Option<&str>) -> Vec<String> {
    let Some(intent) = intent.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    let decision = decision.map(str::trim).unwrap_or("");
    if decision.is_empty() || decision.eq_ignore_ascii_case(intent) {
        return Vec::new();
    }
    vec![format!(
        "Bench intent is {intent}: the Lead decided {decision}"
    )]
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
    if let Some(value) = form.get("surface") {
        let direct = clean(value);
        return (!direct.is_empty()).then_some(direct);
    }
    let slice = form.get("splitBranch")?;
    let plan = slice.get("plan")?;
    let chunks = match plan.get("chunks") {
        Some(Value::Array(items)) => items,
        _ => return None,
    };
    // Workflow attaches the selected branch's zero-based index to each fork
    // task. Collecting every chunk's surface here made each child claim the
    // whole plan, which hid independent lanes from the scheduler.
    let index = usize::try_from(form.get("splitBranchIndex")?.as_i64()?).ok()?;
    chunks.get(index).and_then(|chunk| {
        chunk
            .get("surface")
            .map(clean)
            .filter(|surface| !surface.is_empty())
    })
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
    fn bench_intent_caps_the_lead_decision() {
        // The legacy matrix, verbatim (`agents/handoff.test.ts:29-33`).
        assert!(!bench_intent_errors(Some("SMITH"), Some("HOLD")).is_empty());
        assert!(bench_intent_errors(Some("HOLD"), Some("HOLD")).is_empty());
        assert!(!bench_intent_errors(Some("SPLIT"), Some("SOLO")).is_empty());
        // The phrasing the bench test asserts on, and the cap in both directions.
        assert_eq!(
            bench_intent_errors(Some("HOLD"), Some("SOLO")),
            vec!["Bench intent is HOLD: the Lead decided SOLO".to_string()]
        );
        assert!(!bench_intent_errors(Some("SOLO"), Some("SPLIT")).is_empty());
        assert!(bench_intent_errors(Some("SOLO"), Some("SOLO")).is_empty());
        // No cap, a blank cap, and no decision yet all decide nothing here.
        assert!(bench_intent_errors(None, Some("SPLIT")).is_empty());
        assert!(bench_intent_errors(Some("  "), Some("SPLIT")).is_empty());
        assert!(bench_intent_errors(Some("SMITH"), None).is_empty());
    }

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

    #[test]
    fn a_present_but_empty_surface_does_not_fall_back_to_another_branch() {
        use std::collections::BTreeMap;
        let mut form = BTreeMap::new();
        form.insert("surface".into(), Value::Array(vec![]));
        let mut chunk = BTreeMap::new();
        chunk.insert(
            "surface".into(),
            Value::Array(vec![Value::String("src/other.rs".into())]),
        );
        let mut plan = BTreeMap::new();
        plan.insert("chunks".into(), Value::Array(vec![Value::Object(chunk)]));
        let mut split = BTreeMap::new();
        split.insert("plan".into(), Value::Object(plan));
        form.insert("splitBranchIndex".into(), Value::from(0));
        form.insert("splitBranch".into(), Value::Object(split));
        assert_eq!(forge_lane_surface(Some(&Value::Object(form))), None);
    }

    #[test]
    fn split_surface_uses_only_the_task_branch_index() {
        use std::collections::BTreeMap;
        let chunk = |path: &str| {
            let mut chunk = BTreeMap::new();
            chunk.insert(
                "surface".into(),
                Value::Array(vec![Value::String(path.into())]),
            );
            Value::Object(chunk)
        };
        let mut plan = BTreeMap::new();
        plan.insert(
            "chunks".into(),
            Value::Array(vec![chunk("src/a.rs"), chunk("src/b.rs")]),
        );
        let mut split = BTreeMap::new();
        split.insert("plan".into(), Value::Object(plan.clone()));
        let mut form = BTreeMap::new();
        form.insert("splitBranchIndex".into(), Value::from(1));
        form.insert("splitBranch".into(), Value::Object(split));
        let form = Value::Object(form);
        assert_eq!(
            forge_lane_surface(Some(&form)),
            Some(vec!["src/b.rs".into()])
        );

        let mut split = BTreeMap::new();
        split.insert("plan".into(), Value::Object(plan));
        let mut missing_index = BTreeMap::new();
        missing_index.insert("splitBranch".into(), Value::Object(split));
        let missing_index = Value::Object(missing_index);
        assert_eq!(forge_lane_surface(Some(&missing_index)), None);
    }
}
