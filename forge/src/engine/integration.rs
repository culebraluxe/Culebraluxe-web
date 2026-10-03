//! Port of workflow_app/forge/forge-integration-attestation.ts. Pure except injected probes.

use crate::engine::release_receipt::{ReleaseEvidence, ReleaseReceiptKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildObservation {
    pub command: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationAttestation {
    pub artifact_sha: Option<String>,
    pub integrated_ref: String,
    pub integrated: bool,
    pub build: Option<BuildObservation>,
    pub proofs: Option<Vec<BuildObservation>>,
    pub reason: Option<String>,
}

fn looks_like_sha(s: &str) -> bool {
    let v = s.trim();
    (7..=40).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn attest_integration(
    candidate_sha: Option<&str>,
    integrated_ref: &str,
    is_ancestor: impl Fn(&str, &str) -> Result<bool, String>,
    build: Option<BuildObservation>,
    proofs: Option<Vec<BuildObservation>>,
) -> IntegrationAttestation {
    let base_ref = integrated_ref.to_string();
    let sha = candidate_sha.unwrap_or("").trim().to_string();
    if !looks_like_sha(&sha) {
        return IntegrationAttestation {
            artifact_sha: None,
            integrated_ref: base_ref,
            integrated: false,
            build: build.clone(),
            proofs: proofs.clone(),
            reason: Some(format!("no artifact sha to attest (got {candidate_sha:?})")),
        };
    }
    let integrated = match is_ancestor(&sha, integrated_ref) {
        Ok(v) => v,
        Err(e) => {
            return IntegrationAttestation {
                artifact_sha: Some(sha.clone()),
                integrated_ref: base_ref,
                integrated: false,
                build,
                proofs,
                reason: Some(format!(
                    "could not verify containment of sha in {integrated_ref}: {e}"
                )),
            };
        }
    };
    if !integrated {
        return IntegrationAttestation {
            artifact_sha: Some(sha.clone()),
            integrated_ref: base_ref,
            integrated: false,
            build,
            proofs,
            reason: Some(format!("{sha} is not contained in {integrated_ref}")),
        };
    }
    if build.is_none() && proofs.as_ref().map(|p| p.is_empty()).unwrap_or(true) {
        return IntegrationAttestation {
            artifact_sha: Some(sha),
            integrated_ref: base_ref,
            integrated: true,
            build,
            proofs,
            reason: Some("no build was observed, so no clean-build claim is made".into()),
        };
    }
    if let Some(b) = &build {
        if b.exit_code != 0 {
            // The reason is computed BEFORE `build` is moved into the attestation. Reading `b` after the move is
            // exactly what the borrow checker refused (E0505); one local, identical output.
            let reason = format!("build exited {} ({})", b.exit_code, b.command);
            return IntegrationAttestation {
                artifact_sha: Some(sha.clone()),
                integrated_ref: base_ref,
                integrated: true,
                build,
                proofs,
                reason: Some(reason),
            };
        }
    }
    if let Some(failed) = proofs
        .as_ref()
        .and_then(|p| p.iter().find(|x| x.exit_code != 0))
    {
        // Same shape as the build branch above: the reason is built before `proofs` is moved.
        let reason = format!(
            "frozen proof exited {} ({})",
            failed.exit_code, failed.command
        );
        return IntegrationAttestation {
            artifact_sha: Some(sha.clone()),
            integrated_ref: base_ref,
            integrated: true,
            build,
            proofs,
            reason: Some(reason),
        };
    }
    IntegrationAttestation {
        artifact_sha: Some(sha),
        integrated_ref: base_ref,
        integrated: true,
        build,
        proofs,
        reason: None,
    }
}

pub fn release_evidence_from_integration(
    attestation: &IntegrationAttestation,
) -> Option<ReleaseEvidence> {
    if !attestation.integrated {
        return None;
    }
    let sha = attestation.artifact_sha.as_deref()?;
    let mut observed = Vec::new();
    if let Some(b) = &attestation.build {
        observed.push(b);
    }
    if let Some(proofs) = &attestation.proofs {
        observed.extend(proofs.iter());
    }
    if observed.is_empty() || observed.iter().any(|c| c.exit_code != 0) {
        return None;
    }
    Some(ReleaseEvidence {
        kind: ReleaseReceiptKind::Integration,
        artifact_sha: sha.to_string(),
        receipt_id: format!(
            "integration:{}@{}",
            attestation.integrated_ref,
            &sha[..sha.len().min(12)]
        ),
        success: true,
    })
}

pub fn git_is_ancestor(cwd: &str, sha: &str, r#ref: &str) -> bool {
    std::process::Command::new("git")
        .args(["merge-base", "--is-ancestor", sha, r#ref])
        .current_dir(cwd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_without_observed_proof() {
        let a = attest_integration(
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            "origin/main",
            |_, _| Ok(true),
            None,
            None,
        );
        assert!(a.reason.is_some());
        assert!(release_evidence_from_integration(&a).is_none());
    }
}
