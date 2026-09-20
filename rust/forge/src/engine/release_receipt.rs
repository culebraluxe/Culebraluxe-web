//! Port of workflow_app/forge/forge-release-receipt.ts. Pure.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseReceiptKind {
    Deployment,
    ProductionVerification,
    Integration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseEvidence {
    pub kind: ReleaseReceiptKind,
    pub artifact_sha: String,
    pub receipt_id: String,
    pub success: bool,
}

const PLACEHOLDERS: &[&str] = &[
    "n/a", "na", "none", "null", "nil", "tbd", "todo", "unknown", "placeholder",
    "synthetic", "fake", "mock", "dummy", "test", "manual", "waived", "x", "-",
];

pub fn is_placeholder_receipt_id(receipt_id: Option<&str>) -> bool {
    let value = receipt_id.unwrap_or("").trim().to_ascii_lowercase();
    if value.is_empty() {
        return true;
    }
    if PLACEHOLDERS.contains(&value.as_str()) {
        return true;
    }
    let head = value
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ',' | ':' | '—' | '-'))
        .next()
        .unwrap_or("");
    PLACEHOLDERS.contains(&head)
}

pub fn is_commit_sha(value: Option<&str>) -> bool {
    let v = value.unwrap_or("").trim();
    (7..=40).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptAssessment {
    pub ok: bool,
    pub reason: Option<String>,
}

pub fn assess_release_receipt(receipt: Option<&ReleaseEvidence>) -> ReceiptAssessment {
    let Some(receipt) = receipt else {
        return ReceiptAssessment {
            ok: false,
            reason: Some("no release receipt was provided".into()),
        };
    };
    if !is_commit_sha(Some(&receipt.artifact_sha)) {
        return ReceiptAssessment {
            ok: false,
            reason: Some(format!(
                "release receipt artifactSha is not a commit sha: {:?}",
                receipt.artifact_sha
            )),
        };
    }
    if is_placeholder_receipt_id(Some(&receipt.receipt_id)) {
        return ReceiptAssessment {
            ok: false,
            reason: Some(format!(
                "release receipt id is a placeholder, not a receipt: {:?}",
                receipt.receipt_id
            )),
        };
    }
    ReceiptAssessment { ok: true, reason: None }
}

pub fn deployment_receipt_failure_reason(
    receipt: Option<&ReleaseEvidence>,
    published_sha: Option<&str>,
) -> Option<String> {
    let assessment = assess_release_receipt(receipt);
    if !assessment.ok {
        return assessment.reason;
    }
    if !is_commit_sha(published_sha) {
        return Some("no published artifact sha was recorded to compare against".into());
    }
    let receipt = receipt.unwrap();
    let published = published_sha.unwrap().trim().to_ascii_lowercase();
    let artifact = receipt.artifact_sha.trim().to_ascii_lowercase();
    if published != artifact && !published.starts_with(&artifact) && !artifact.starts_with(&published) {
        return Some(format!(
            "published sha {published} does not match deployed artifact {}",
            receipt.artifact_sha
        ));
    }
    if !receipt.success {
        return Some("release receipt success is false".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_placeholder() {
        assert!(is_placeholder_receipt_id(Some("n/a")));
        assert!(is_placeholder_receipt_id(Some("tbd - see memo")));
        assert!(!is_placeholder_receipt_id(Some("deploy:abc1234")));
    }
}
