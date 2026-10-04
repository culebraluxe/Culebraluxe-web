//! Stop-after dispatch cap parsing and resolution.

use std::collections::BTreeSet;

use crate::engine::service_binding::nodes_for_service;

/// The dispatch cap the worker carries off the claimed row, as a stop target
/// (migration 167: `scout` | `architect` | `lead`, NULL = the full chain).
///
/// The column existed and no Rust reader honoured it: the worker parsed the same
/// three words out of argv and the engine binary handed `stop_after: None` to
/// every run, so a dispatch that asked to stop at the architect ran the whole
/// chain. `None` here is returned only for a word this does not recognise — the
/// caller refuses it instead of quietly widening the run to the full chain.
pub fn parse_forge_stop_after(raw: &str) -> Option<ForgeStopTarget> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "scout" => Some(ForgeStopTarget::Role("scout")),
        "architect" => Some(ForgeStopTarget::Role("architect")),
        "lead" => Some(ForgeStopTarget::Role("lead")),
        _ => None,
    }
}

/// The node a `stop_after: lead` cap stops at.
///
/// The only node name the drive still holds, and it is one because the cap is
/// not a lane's group: `forge.lead` binds four nodes (the pre and post
/// decisions, the SOLO implement, and the failure classifier), while "stop
/// after the lead" means after the PRE decision — the node the next phase is
/// chosen at. The scout and architect caps are the definition's own groups
/// (see [`resolve_forge_stop_target`]), so this is the asymmetry left over,
/// not a table.
const LEAD_PRE_NODE: &str = "lead_pre";

/// Resolve a stop target to the set of node IDs that end the run.
pub fn resolve_forge_stop_target(stop: Option<&ForgeStopTarget>) -> Option<BTreeSet<String>> {
    /// One service's nodes, as the set of nodes whose task ends the run.
    fn cap(service_key: &str) -> BTreeSet<String> {
        nodes_for_service(service_key)
            .into_iter()
            .map(String::from)
            .collect()
    }
    match stop {
        None => None,
        Some(ForgeStopTarget::Node(n)) => Some(BTreeSet::from([n.clone()])),
        Some(ForgeStopTarget::Role("scout")) => Some(cap("forge.scout")),
        Some(ForgeStopTarget::Role("architect")) => Some(cap("forge.architect")),
        Some(ForgeStopTarget::Role("lead")) => Some(BTreeSet::from([LEAD_PRE_NODE.to_string()])),
        Some(ForgeStopTarget::Role(_)) => None,
    }
}

#[derive(Debug, Clone)]
pub enum ForgeStopTarget {
    Role(&'static str),
    Node(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three words the column allows (migration 167) map to the three stop
    /// targets the driver understands.
    #[test]
    fn the_three_allowed_caps_parse_and_nothing_else_does() {
        for (raw, expected) in [
            ("scout", "scout"),
            ("architect", "architect"),
            ("lead", "lead"),
            (" ARCHITECT ", "architect"),
        ] {
            match parse_forge_stop_after(raw) {
                Some(ForgeStopTarget::Role(role)) => assert_eq!(role, expected, "{raw}"),
                other => panic!("{raw} parsed as {other:?}"),
            }
        }
        // A cap that cannot be read must never widen to the full chain: the caller refuses these.
        for raw in ["", "smith", "FEATURE", "all", "0"] {
            assert!(
                parse_forge_stop_after(raw).is_none(),
                "{raw} must not parse into a cap"
            );
        }
    }

    /// Each cap resolves to the nodes the driver stops after, and a cap nobody
    /// defined stops nothing.
    #[test]
    fn a_cap_resolves_to_its_own_nodes() {
        let scout = resolve_forge_stop_target(Some(&ForgeStopTarget::Role("scout"))).unwrap();
        assert!(scout.contains("feature_scout"));
        let architect =
            resolve_forge_stop_target(Some(&ForgeStopTarget::Role("architect"))).unwrap();
        assert!(architect.contains("architect"));
        let lead = resolve_forge_stop_target(Some(&ForgeStopTarget::Role("lead"))).unwrap();
        assert!(lead.contains("lead_pre"));
        assert!(
            resolve_forge_stop_target(None).is_none(),
            "NULL is the full chain"
        );
        assert!(resolve_forge_stop_target(Some(&ForgeStopTarget::Role("unknown"))).is_none());
    }
}