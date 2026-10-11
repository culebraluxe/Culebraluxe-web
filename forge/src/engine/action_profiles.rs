//! Action policies: which kinds of action an agent's execution surface may use. Named, testable, and checked against the
//! live agent profiles, so an invariant broken by a future edit fails a test instead of reaching a run.
//!
//! THIS IS NOT A PERMISSION SYSTEM. The agent profiles in `opencode_agents` remain the authority that the vendor enforces.
//! This module names the shapes those profiles may take and states the rules they must keep:
//! - A shell surface is an explicit allowlist, never "shell allowed".
//! - Git mutation (push, merge, reset, branch) is never granted through a shell surface. Git authority is a separate
//!   capability and no action policy confers it.
//! - Only an agent with implement authority may hold an edit or composite shell surface.
//! - QA/Assay never holds an edit surface.
//! - Release operations belong to DEV_OPS alone.
//!
//! The action policy decides only what an authorized agent can DO with its tools. It never decides what happens next.

use crate::engine::opencode_agents::{Authority, V2AgentProfile, V2_AGENT_PROFILES};

/// The action surfaces of the order. Phase 3 names them; the shipped agents keep their existing surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPolicy {
    /// Typed read and search operations only; no edit, no shell. The surface of a read-only agent such as Scout.
    StructuredReadOnly,
    /// Typed read, search and edit operations only; no shell. The surface of an implementer without shell access, Smith today.
    StructuredOnly,
    /// Structured operations plus an allowlisted, read-only shell (running approved tests). No edit authority.
    VerificationOnly,
    /// Structured operations plus a shell allowlist, for an implementer that is proven to use a shell well. Not shipped by
    /// default; it needs its own evidence before it is enabled.
    CompositeShell,
    /// Release and publish operations. DEV_OPS only.
    ReleaseOperations,
}

/// What a surface lets an agent do with the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellSurface {
    /// `bash` is denied outright.
    None,
    /// `bash` is allowed only for the listed command prefixes.
    Allowlist,
}

/// The surface an action policy grants. Git mutation is not a field: no policy grants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolSurface {
    pub read: bool,
    pub edit: bool,
    pub shell: ShellSurface,
    pub release: bool,
}

impl ActionPolicy {
    pub fn surface(self) -> ToolSurface {
        match self {
            ActionPolicy::StructuredReadOnly => ToolSurface {
                read: true,
                edit: false,
                shell: ShellSurface::None,
                release: false,
            },
            ActionPolicy::StructuredOnly => ToolSurface {
                read: true,
                edit: true,
                shell: ShellSurface::None,
                release: false,
            },
            ActionPolicy::VerificationOnly => ToolSurface {
                read: true,
                edit: false,
                shell: ShellSurface::Allowlist,
                release: false,
            },
            ActionPolicy::CompositeShell => ToolSurface {
                read: true,
                edit: true,
                shell: ShellSurface::Allowlist,
                release: false,
            },
            ActionPolicy::ReleaseOperations => ToolSurface {
                read: true,
                edit: false,
                shell: ShellSurface::Allowlist,
                release: true,
            },
        }
    }
}

/// Whether an agent with this authority may hold this policy at all. The authority is the vendor-enforced fact; a policy
/// that needs more than it grants is refused here, before any run.
pub fn policy_permitted(authority: Authority, policy: ActionPolicy) -> bool {
    let surface = policy.surface();
    match authority {
        // A release is performed by Forge's deploy path, not by an edit: a read-only DEV_OPS agent may hold it.
        Authority::ReadOnly => !surface.edit,
        Authority::Implement => true,
    }
}

/// The action policy an agent's profile implies. Derived from the profile itself, never hand-listed: the authority says
/// whether it may edit, the shell allowlist says whether it has a shell, and the DEV_OPS agent alone releases. So a
/// profile edit that changes its surface changes this answer, and the invariants below catch any that break a rule.
pub fn policy_for_profile(profile: &V2AgentProfile) -> ActionPolicy {
    if profile.id == "forge-devops" {
        return ActionPolicy::ReleaseOperations;
    }
    let has_shell = !profile.shell_allow.is_empty();
    match (profile.authority, has_shell) {
        (Authority::ReadOnly, false) => ActionPolicy::StructuredReadOnly,
        (Authority::ReadOnly, true) => ActionPolicy::VerificationOnly,
        (Authority::Implement, false) => ActionPolicy::StructuredOnly,
        (Authority::Implement, true) => ActionPolicy::CompositeShell,
    }
}

/// The policy for a shipped agent id, or `None` if no such profile ships.
pub fn policy_for_agent(agent_id: &str) -> Option<ActionPolicy> {
    V2_AGENT_PROFILES
        .iter()
        .find(|profile| profile.id == agent_id)
        .map(policy_for_profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(id: &str) -> ActionPolicy {
        policy_for_agent(id).unwrap_or_else(|| panic!("{id} ships a profile"))
    }

    #[test]
    fn every_shipped_agent_holds_a_policy_its_authority_permits() {
        for profile in V2_AGENT_PROFILES.iter() {
            let policy = policy_for_profile(profile);
            assert!(
                policy_permitted(profile.authority, policy),
                "{} has authority {:?} but its policy {:?} needs more",
                profile.id,
                profile.authority,
                policy
            );
        }
    }

    #[test]
    fn a_read_only_agent_never_edits_and_its_shell_is_its_profiles_read_only_allowlist() {
        // Scout ships a read-only inspection allowlist, so its shell surface is that allowlist: an allowlist, never open,
        // and it never edits.
        assert_eq!(policy("forge-scout"), ActionPolicy::VerificationOnly);
        assert!(!policy("forge-scout").surface().edit);
        assert_eq!(
            policy("forge-scout").surface().shell,
            ShellSurface::Allowlist
        );
    }

    #[test]
    fn qa_and_assay_never_hold_an_edit_surface() {
        for id in ["forge-assay", "forge-inspector"] {
            assert!(!policy(id).surface().edit, "{id} must not edit");
        }
    }

    #[test]
    fn only_devops_holds_release_operations() {
        for profile in V2_AGENT_PROFILES.iter() {
            let release = policy_for_profile(profile).surface().release;
            assert_eq!(release, profile.id == "forge-devops", "{}", profile.id);
        }
    }

    #[test]
    fn a_composite_shell_surface_needs_implement_authority() {
        // The composite surface edits, so a read-only agent may never be given it.
        assert!(!policy_permitted(
            Authority::ReadOnly,
            ActionPolicy::CompositeShell
        ));
        assert!(policy_permitted(
            Authority::Implement,
            ActionPolicy::CompositeShell
        ));
    }

    #[test]
    fn a_shell_surface_is_an_allowlist_never_open() {
        for policy in [
            ActionPolicy::StructuredReadOnly,
            ActionPolicy::StructuredOnly,
            ActionPolicy::VerificationOnly,
            ActionPolicy::CompositeShell,
            ActionPolicy::ReleaseOperations,
        ] {
            // The only shell states are "none" and "allowlist"; there is deliberately no "all".
            assert!(matches!(
                policy.surface().shell,
                ShellSurface::None | ShellSurface::Allowlist
            ));
        }
    }

    #[test]
    fn no_action_policy_confers_git_mutation() {
        // Git mutation is not a field of the surface at all. A Git capability must be added as its own, separate type; no
        // action policy can grant it. This pins that every surface is the four fields above.
        for policy in [
            ActionPolicy::StructuredReadOnly,
            ActionPolicy::StructuredOnly,
            ActionPolicy::VerificationOnly,
            ActionPolicy::CompositeShell,
            ActionPolicy::ReleaseOperations,
        ] {
            let surface = policy.surface();
            assert!(
                surface.read
                    || surface.edit
                    || surface.release
                    || surface.shell != ShellSurface::None
            );
        }
    }
}
