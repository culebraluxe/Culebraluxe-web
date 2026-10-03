//! Forge command inventory — the B fork of Layer 4.
//! RE commands (deal.*, offer.*, task.*) never live here.

pub const FORGE_COMMAND_NAMESPACE: &str = "forge";

pub const STORY_MARK_HOLD: &str = "forge.story.hold";
pub const STORY_MARK_COMPLETE: &str = "forge.story.complete";
pub const STORY_MARK_IN_PROGRESS: &str = "forge.story.in_progress";
pub const RUN_APPEND_DETAIL: &str = "forge.run.detail";

pub const RUN_SCOUT: &str = "forge.run_scout";
pub const RUN_DIAGNOSIS: &str = "forge.run_diagnosis";
pub const RUN_ARCHITECT: &str = "forge.run_architect";
pub const RUN_LEAD_PRE: &str = "forge.run_lead_pre";
pub const RUN_LEAD_IMPLEMENT: &str = "forge.run_lead_implement";
pub const RUN_SMITH: &str = "forge.run_smith";
pub const RUN_SMITH_SPLIT: &str = "forge.run_smith_split";
pub const RUN_LEAD_POST: &str = "forge.run_lead_post";
pub const RUN_QA_REVIEW: &str = "forge.run_qa_review";
pub const RUN_QA_VERIFY: &str = "forge.run_qa_verify";
pub const CLASSIFY_FAILURE: &str = "forge.classify_failure";
pub const RUN_SMITH_REPAIR: &str = "forge.run_smith_repair";
pub const RUN_ARCHITECT_REPAIR: &str = "forge.run_architect_repair";
pub const RUN_DEVOPS_REPAIR: &str = "forge.run_devops_repair";
pub const PUBLISH_CANDIDATE: &str = "forge.publish_candidate";
pub const MIGRATE_DEV: &str = "forge.migrate_dev";
pub const VERIFY_DEV_MIGRATION: &str = "forge.verify_dev_migration";
pub const MIGRATE_PROD: &str = "forge.migrate_prod";
pub const VERIFY_PROD_MIGRATION: &str = "forge.verify_prod_migration";
pub const REFRESH_DERIVED_MODELS: &str = "forge.refresh_derived_models";
pub const VERIFY_DERIVED_MODELS: &str = "forge.verify_derived_models";
pub const DEPLOY: &str = "forge.deploy";
pub const VERIFY_PRODUCTION: &str = "forge.verify_production";

pub const XML_RELEASE_COMMANDS: &[&str] = &[
    PUBLISH_CANDIDATE,
    MIGRATE_DEV,
    VERIFY_DEV_MIGRATION,
    MIGRATE_PROD,
    VERIFY_PROD_MIGRATION,
    REFRESH_DERIVED_MODELS,
    VERIFY_DERIVED_MODELS,
];

pub const ROUTED: &[&str] = &[
    STORY_MARK_HOLD,
    STORY_MARK_COMPLETE,
    STORY_MARK_IN_PROGRESS,
    RUN_APPEND_DETAIL,
    RUN_SCOUT,
    RUN_DIAGNOSIS,
    RUN_ARCHITECT,
    RUN_LEAD_PRE,
    RUN_LEAD_IMPLEMENT,
    RUN_SMITH,
    RUN_SMITH_SPLIT,
    RUN_LEAD_POST,
    RUN_QA_REVIEW,
    RUN_QA_VERIFY,
    CLASSIFY_FAILURE,
    RUN_SMITH_REPAIR,
    RUN_ARCHITECT_REPAIR,
    RUN_DEVOPS_REPAIR,
    PUBLISH_CANDIDATE,
    MIGRATE_DEV,
    VERIFY_DEV_MIGRATION,
    MIGRATE_PROD,
    VERIFY_PROD_MIGRATION,
    REFRESH_DERIVED_MODELS,
    VERIFY_DERIVED_MODELS,
    DEPLOY,
    VERIFY_PRODUCTION,
];

pub fn is_routed(command_type: &str) -> bool {
    ROUTED.contains(&command_type)
}

pub fn is_release(command_type: &str) -> bool {
    XML_RELEASE_COMMANDS.contains(&command_type)
}

pub fn is_forge_namespace(command_type: &str) -> bool {
    command_type.starts_with("forge.")
}
