//! Approved, versioned assay plan contract.
//!
//! The Storyboard plan is a proposal until the operator approval columns are set.  A plan is then
//! frozen on the Story Run and this module validates its identity and references before QA executes
//! any command.  Legacy prose is deliberately not converted into assertions here.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const ASSAY_PLAN_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovedAssayPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub plan_version: u32,
    pub commands: Vec<ApprovedAssayCommand>,
    pub checks: Vec<ApprovedAssertionCheck>,
    pub conditions: Vec<ApprovedAcceptanceCondition>,
    #[serde(default)]
    pub negative_control: Option<ApprovedNegativeControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovedAssayCommand {
    pub id: String,
    /// Must exactly match a command in the authoritative packet snapshot.
    pub command: String,
    pub runner: AssayRunner,
    pub parser: AssayParser,
    /// Current Forge execution port always runs in the lane root. This is named explicitly so a
    /// future expansion cannot silently change cwd semantics.
    pub working_directory: String,
    /// Opaque, non-secret identity for the approved environment policy. Values are never stored.
    pub environment_identity: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssayRunner {
    RustLibtest,
    Tap,
    Junit,
    CommandExit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssayParser {
    RustLibtest,
    Tap,
    Junit,
    CommandExit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovedAssertionCheck {
    pub id: String,
    pub command_id: String,
    /// Stable runner assertion reference, optionally `relative/file#test name`.
    pub assertion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovedAcceptanceCondition {
    pub id: String,
    pub check_ids: Vec<String>,
    pub aggregation: CheckAggregation,
    #[serde(default)]
    pub judgment: AcceptanceJudgment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceJudgment {
    #[default]
    Product,
    TestArtifact,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckAggregation {
    AllRequired,
    AnyOf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovedNegativeControl {
    pub command_id: String,
    pub expected_failed_check_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanIdentity {
    pub hash: String,
    pub schema_version: u32,
    pub plan_id: String,
    pub plan_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPlanSnapshot {
    pub plan: ApprovedAssayPlan,
    pub identity: PlanIdentity,
    pub approved_by: String,
    pub approved_at: String,
}

/// Decode and validate a run snapshot. Missing, unapproved, or stale approval is a visible
/// plan-required state; none of it falls back to acceptance prose or transport booleans.
pub fn validate_frozen_snapshot(
    snapshot: &serde_json::Value,
    story_id: &str,
    packet_commands: &[String],
) -> Result<ValidatedPlanSnapshot, Vec<String>> {
    let mut errors = Vec::new();
    let plan_value = snapshot
        .get("plan")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let plan: ApprovedAssayPlan = match serde_json::from_value(plan_value) {
        Ok(plan) => plan,
        Err(error) => return Err(vec![format!("assay plan missing or malformed: {error}")]),
    };
    if let Err(mut validation) = plan.validate(story_id, packet_commands) {
        errors.append(&mut validation);
    }
    let approved_by = snapshot
        .get("approved_by")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let approved_at = snapshot
        .get("approved_at")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let approved_hash = snapshot
        .get("approved_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if approved_by.is_empty() || approved_at.is_empty() || approved_hash.is_empty() {
        errors.push("assay plan is not operator-approved".into());
    }
    let identity = match plan.identity() {
        Ok(identity) => identity,
        Err(error) => return Err(vec![error]),
    };
    if !approved_hash.is_empty() && approved_hash != identity.hash {
        errors.push("approved assay plan hash does not match frozen plan".into());
    }
    if errors.is_empty() {
        Ok(ValidatedPlanSnapshot {
            plan,
            identity,
            approved_by,
            approved_at,
        })
    } else {
        Err(errors)
    }
}

impl ApprovedAssayPlan {
    pub fn validate(&self, story_id: &str, packet_commands: &[String]) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.schema_version != ASSAY_PLAN_SCHEMA_VERSION {
            errors.push(format!(
                "unsupported assay plan schema {}",
                self.schema_version
            ));
        }
        if self.plan_id.trim().is_empty() || self.plan_version == 0 {
            errors.push("assay plan identity/version is missing".into());
        }
        if self.commands.is_empty() || self.checks.is_empty() || self.conditions.is_empty() {
            errors.push("assay plan requires commands, checks, and acceptance conditions".into());
        }
        if !self
            .conditions
            .iter()
            .any(|condition| condition.judgment == AcceptanceJudgment::Product)
        {
            errors.push("assay plan requires at least one product acceptance condition".into());
        }
        let mut command_ids = HashSet::new();
        let mut command_texts = HashSet::new();
        for command in &self.commands {
            if command.id.trim().is_empty() || !command_ids.insert(command.id.as_str()) {
                errors.push(format!(
                    "empty or duplicate assay command id: {}",
                    command.id
                ));
            }
            if command.command.trim().is_empty() || !command_texts.insert(command.command.as_str())
            {
                errors.push(format!("empty or duplicate assay command: {}", command.id));
            }
            if !packet_commands
                .iter()
                .any(|approved| approved == &command.command)
            {
                errors.push(format!(
                    "assay command {} is not in the frozen Story Packet",
                    command.id
                ));
            }
            if command.working_directory != "lane_root" {
                errors.push(format!(
                    "unsupported working directory for command {}",
                    command.id
                ));
            }
            if command.environment_identity.trim().is_empty() {
                errors.push(format!(
                    "missing environment identity for command {}",
                    command.id
                ));
            }
            if command.runner == AssayRunner::CommandExit
                && crate::engine::assay::is_rust_contract_runtime_test(&command.command)
            {
                errors.push(format!(
                    "test command {} must use a test assertion parser, not command-exit proof",
                    command.id
                ));
            }
            let compatible = matches!(
                (command.runner, command.parser),
                (AssayRunner::RustLibtest, AssayParser::RustLibtest)
                    | (AssayRunner::Tap, AssayParser::Tap)
                    | (AssayRunner::Junit, AssayParser::Junit)
                    | (AssayRunner::CommandExit, AssayParser::CommandExit)
            );
            if !compatible {
                errors.push(format!("runner/parser mismatch for command {}", command.id));
            }
        }

        let mut check_ids = HashSet::new();
        for check in &self.checks {
            if check.id.trim().is_empty() || !check_ids.insert(check.id.as_str()) {
                errors.push(format!(
                    "empty or duplicate assertion check id: {}",
                    check.id
                ));
            }
            if !command_ids.contains(check.command_id.as_str()) {
                errors.push(format!(
                    "check {} references missing command {}",
                    check.id, check.command_id
                ));
            }
            if check.assertion.trim().is_empty() {
                errors.push(format!(
                    "check {} has an empty assertion reference",
                    check.id
                ));
            }
            if self
                .commands
                .iter()
                .find(|command| command.id == check.command_id)
                .is_some_and(|command| command.runner == AssayRunner::CommandExit)
                && check.assertion != "exit_code_zero"
            {
                errors.push(format!(
                    "command-exit check {} must use the exit_code_zero evidence contract",
                    check.id
                ));
            }
        }

        let mut condition_ids = HashSet::new();
        for condition in &self.conditions {
            if condition.id.trim().is_empty() || !condition_ids.insert(condition.id.as_str()) {
                errors.push(format!(
                    "empty or duplicate acceptance condition id: {}",
                    condition.id
                ));
            }
            if condition.check_ids.is_empty() {
                errors.push(format!("condition {} has no required checks", condition.id));
            }
            let mut refs = HashSet::new();
            for check_id in &condition.check_ids {
                if !refs.insert(check_id.as_str()) {
                    errors.push(format!(
                        "condition {} repeats check {}",
                        condition.id, check_id
                    ));
                }
                if !check_ids.contains(check_id.as_str()) {
                    errors.push(format!(
                        "condition {} references missing check {}",
                        condition.id, check_id
                    ));
                }
            }
        }
        if let Some(control) = &self.negative_control {
            if !command_ids.contains(control.command_id.as_str()) {
                errors.push(format!(
                    "negative control references missing command {}",
                    control.command_id
                ));
            }
            if control.expected_failed_check_ids.is_empty() {
                errors.push("negative control must name expected failed checks".into());
            }
            for check_id in &control.expected_failed_check_ids {
                if !check_ids.contains(check_id.as_str()) {
                    errors.push(format!(
                        "negative control references missing check {check_id}"
                    ));
                } else if self
                    .checks
                    .iter()
                    .find(|check| check.id == *check_id)
                    .is_some_and(|check| check.command_id != control.command_id)
                {
                    errors.push(format!(
                        "negative-control check {check_id} belongs to another command"
                    ));
                }
            }
            for condition in &self.conditions {
                if condition
                    .check_ids
                    .iter()
                    .any(|check_id| control.expected_failed_check_ids.contains(check_id))
                {
                    errors.push(format!(
                        "negative-control checks cannot also satisfy acceptance condition {}",
                        condition.id
                    ));
                }
            }
        }
        if story_id.trim().is_empty() {
            errors.push("story identity is missing".into());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn identity(&self) -> Result<PlanIdentity, String> {
        let canonical =
            serde_json::to_vec(self).map_err(|e| format!("serialize assay plan: {e}"))?;
        let hash = Sha256::digest(canonical)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(PlanIdentity {
            hash,
            schema_version: self.schema_version,
            plan_id: self.plan_id.clone(),
            plan_version: self.plan_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> ApprovedAssayPlan {
        ApprovedAssayPlan {
            schema_version: 1,
            plan_id: "plan-1".into(),
            plan_version: 1,
            commands: vec![ApprovedAssayCommand {
                id: "cmd-test".into(),
                command: "cargo test -p app".into(),
                runner: AssayRunner::RustLibtest,
                parser: AssayParser::RustLibtest,
                working_directory: "lane_root".into(),
                environment_identity: "forge-inherited-shell-v1".into(),
            }],
            checks: vec![ApprovedAssertionCheck {
                id: "check-1".into(),
                command_id: "cmd-test".into(),
                assertion: "app::required behavior".into(),
            }],
            conditions: vec![ApprovedAcceptanceCondition {
                id: "AC-1".into(),
                check_ids: vec!["check-1".into()],
                aggregation: CheckAggregation::AllRequired,
                judgment: AcceptanceJudgment::Product,
            }],
            negative_control: None,
        }
    }

    #[test]
    fn validates_command_identity_and_references() {
        let plan = plan();
        assert!(plan
            .validate("STORY-1", &["cargo test -p app".into()])
            .is_ok());
        assert!(plan.validate("STORY-1", &["cargo test".into()]).is_err());
    }

    #[test]
    fn plan_hash_changes_with_assertion_mapping() {
        let mut a = plan();
        let before = a.identity().unwrap();
        a.checks[0].assertion = "app::other behavior".into();
        assert_ne!(before.hash, a.identity().unwrap().hash);
    }

    #[test]
    fn unknown_fields_and_wrong_types_are_rejected() {
        let value = serde_json::to_value(plan()).unwrap();
        let mut bad = value.clone();
        bad["surprise"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ApprovedAssayPlan>(bad).is_err());
        let mut wrong = value;
        wrong["schema_version"] = serde_json::json!("1");
        assert!(serde_json::from_value::<ApprovedAssayPlan>(wrong).is_err());
    }

    #[test]
    fn negative_control_checks_cannot_be_acceptance_checks() {
        let mut plan = plan();
        plan.negative_control = Some(ApprovedNegativeControl {
            command_id: "cmd-test".into(),
            expected_failed_check_ids: vec!["check-1".into()],
        });
        let errors = plan
            .validate("STORY-1", &["cargo test -p app".into()])
            .expect_err("control failure must not also fail ordinary acceptance");
        assert!(errors
            .iter()
            .any(|error| error.contains("cannot also satisfy acceptance")));
    }
}
