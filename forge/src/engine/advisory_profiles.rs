//! Forge-level advisory profile policy for the Phase 2 executive/expert swarm.
//!
//! `engine/executive_swarm.rs` is the canonical Phase 2 contract: IDs, routing, Council gates,
//! limits, verdicts, synthesis and the Forge handoff. This module is the **one** place that gives
//! each of those agents its execution policy — skills, step budget and read-only posture — so the
//! OpenCode, Maestro and DSH materializers all render from the same source instead of re-authoring
//! a profile per transport.
//!
//! Rules that must not drift:
//! - every profile is read-only; Forge executes, the swarm decides;
//! - skills are existing canonical skill ids (`docs/agent/skills/`), never a second catalogue;
//! - the step budget is bounded well below a role turn: an advisory turn is a question, not a story;
//! - web access is granted only where research genuinely needs it.

use crate::engine::executive_swarm::SwarmAgent;

/// Step ceiling for one advisory turn. Deliberately below `ROLE_MAX_STEPS`: an advisor reasons and
/// reports; it does not implement.
pub const ADVISORY_MAX_STEPS: u32 = 24;

/// Execution policy for one Phase 2 profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvisoryProfile {
    /// Canonical Phase 2 id (`exec-…` / `expert-…`).
    pub id: &'static str,
    pub description: &'static str,
    /// Canonical skill ids this profile may load.
    pub skills: &'static [&'static str],
    pub steps: u32,
    /// Read-only by construction; a materializer must refuse to render anything else.
    pub read_only: bool,
    /// Whether the profile may use read-only web research.
    pub web: bool,
}

/// The twelve profiles, in catalogue order: four executives, then eight experts.
pub const ADVISORY_PROFILES: [AdvisoryProfile; 12] = [
    AdvisoryProfile {
        id: "exec-chief-of-staff",
        description: "Coordinates the panel and synthesizes one decision from independent verdicts; never implements.",
        skills: &["planner", "workflow"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "exec-advisor",
        description: "Strategic judgment: should we do it, what is the best choice, what are the trade-offs.",
        skills: &["planner", "ripwire", "workflow"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: true,
    },
    AdvisoryProfile {
        id: "exec-principal-architect",
        description: "Cross-system architecture authority: boundaries, ownership, durable architectural policy.",
        skills: &[
            "abstract-service",
            "planner",
            "ripwire",
            "rust-yew-mvi",
            "security-entitlements",
            "ui",
            "workflow",
        ],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: true,
    },
    AdvisoryProfile {
        id: "exec-adversary",
        description: "Dissent seat: why might the favored answer be wrong; which premise is unmeasured.",
        skills: &["abstract-service", "ripwire", "security-entitlements", "semgrep"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: true,
    },
    AdvisoryProfile {
        id: "expert-security",
        description: "Security and entitlements: boundaries, grants, injection, secrets and policy relaxation.",
        skills: &["abstract-service", "security-entitlements", "semgrep"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "expert-database",
        description: "Database and migration safety: schema, data integrity, Neon branches and migrations.",
        skills: &["abstract-service", "database-migration", "neon"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "expert-rust",
        description: "Rust implementation and testing judgment: type design, tests, lints and crate boundaries.",
        skills: &["abstract-service", "ripwire", "rust-testing"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "expert-ui-mvi",
        description: "UI and MVI: screen state, reducer shape, brand and accessibility.",
        skills: &["rust-testing", "rust-yew-mvi", "ui"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "expert-devops",
        description: "Delivery and operations: release state, harness health, deployment constraints.",
        skills: &["neon", "rust-testing", "workflow"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "expert-integration",
        description: "Integration: contract seams, external adapters, rollout compatibility.",
        skills: &["abstract-service", "semgrep", "workflow"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "expert-performance",
        description: "Performance: measured cost, hotspots, allocation and query shape.",
        skills: &["ripwire", "rtk", "rust-testing"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: false,
    },
    AdvisoryProfile {
        id: "expert-research",
        description: "Research: options, prior art, measurement plans and what would change our mind.",
        skills: &["planner", "ripwire", "serena"],
        steps: ADVISORY_MAX_STEPS,
        read_only: true,
        web: true,
    },
];

/// The profile for a Phase 2 agent. Unknown ids fail closed at the materializer.
pub fn advisory_profile(agent: SwarmAgent) -> Option<&'static AdvisoryProfile> {
    let id = agent.id();
    ADVISORY_PROFILES.iter().find(|profile| profile.id == id)
}

/// Every canonical Phase 2 id, from the contract itself (never a second hand-written list).
pub fn advisory_ids() -> Vec<&'static str> {
    let executives = crate::engine::executive_swarm::EXECUTIVE_AGENT_CATALOG
        .iter()
        .map(|agent| agent.id());
    let experts = crate::engine::executive_swarm::EXPERT_AGENT_CATALOG
        .iter()
        .map(|agent| agent.id());
    executives.chain(experts).collect()
}

/// A materializer's read-only guard: it must refuse to render a writable advisory profile.
pub fn assert_read_only(profile: &AdvisoryProfile) -> Result<(), String> {
    if profile.read_only {
        Ok(())
    } else {
        Err(format!(
            "advisory profile {} is writable; the Phase 2 contract requires read-only",
            profile.id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::executive_swarm::{ExecutiveAgent, ExpertAgent};

    #[test]
    fn every_contract_id_has_exactly_one_profile() {
        let ids = advisory_ids();
        assert_eq!(ids.len(), 12);
        for id in ids {
            let matching: Vec<&AdvisoryProfile> = ADVISORY_PROFILES
                .iter()
                .filter(|profile| profile.id == id)
                .collect();
            assert_eq!(
                matching.len(),
                1,
                "profile for {id} must exist exactly once"
            );
        }
        assert_eq!(ADVISORY_PROFILES.len(), 12);
    }

    #[test]
    fn every_profile_is_read_only_and_bounded() {
        for profile in &ADVISORY_PROFILES {
            assert!(profile.read_only, "{} must be read-only", profile.id);
            assert_eq!(profile.steps, ADVISORY_MAX_STEPS);
            assert!(!profile.description.is_empty());
        }
    }

    #[test]
    fn every_skill_is_a_known_canonical_skill() {
        let known = crate::engine::opencode_agents::all_skill_ids();
        for profile in &ADVISORY_PROFILES {
            for skill in profile.skills {
                assert!(
                    known.contains(skill),
                    "{} names unknown skill {skill}",
                    profile.id
                );
            }
        }
    }

    #[test]
    fn web_access_is_limited_to_research_roles() {
        let web: Vec<&str> = ADVISORY_PROFILES
            .iter()
            .filter(|profile| profile.web)
            .map(|profile| profile.id)
            .collect();
        assert_eq!(
            web,
            vec![
                "exec-advisor",
                "exec-principal-architect",
                "exec-adversary",
                "expert-research"
            ]
        );
    }

    #[test]
    fn catalogue_lookup_matches_the_contract() {
        let chain = advisory_ids();
        for (index, agent) in crate::engine::executive_swarm::EXECUTIVE_AGENT_CATALOG
            .iter()
            .map(|agent| SwarmAgent::Executive(*agent))
            .chain(
                crate::engine::executive_swarm::EXPERT_AGENT_CATALOG
                    .iter()
                    .map(|agent| SwarmAgent::Expert(*agent)),
            )
            .enumerate()
        {
            let profile = advisory_profile(agent).expect("profile");
            assert_eq!(profile.id, chain[index]);
        }
        assert_eq!(
            advisory_profile(SwarmAgent::Executive(ExecutiveAgent::ChiefOfStaff))
                .map(|profile| profile.id),
            Some("exec-chief-of-staff")
        );
        assert_eq!(
            advisory_profile(SwarmAgent::Expert(ExpertAgent::Research)).map(|profile| profile.id),
            Some("expert-research")
        );
    }
}
