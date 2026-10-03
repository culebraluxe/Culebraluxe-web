//! Port of workflow_app/command-types.ts.

pub const DEAL_SET_STAGE_UNDER_CONTRACT: &str = "deal.set_stage_under_contract";
pub const DEAL_SET_STAGE_CLOSED: &str = "deal.set_stage_closed";
pub const DEAL_SET_CLOSING_DATE: &str = "deal.set_closing_date";
pub const DEAL_SET_FINANCING_TYPE: &str = "deal.set_financing_type";
pub const DEAL_SET_APPRAISAL_REQUIRED: &str = "deal.set_appraisal_required";
pub const DEAL_SET_LENDER_CLEAR_TO_CLOSE: &str = "deal.set_lender_clear_to_close";
pub const DEAL_SET_INSPECTION_DEADLINE: &str = "deal.set_inspection_deadline";
pub const DEAL_SET_FINANCING_DEADLINE: &str = "deal.set_financing_deadline";
pub const OFFER_ACCEPT: &str = "offer.accept";
pub const TASK_CREATE: &str = "task.create";
pub const TASK_COMPLETE: &str = "task.complete";
pub const TASK_CANCEL: &str = "task.cancel";

pub const XML_COMMAND_NODE_TYPES: &[&str] = &[
    DEAL_SET_STAGE_UNDER_CONTRACT,
    DEAL_SET_STAGE_CLOSED,
    DEAL_SET_CLOSING_DATE,
    DEAL_SET_INSPECTION_DEADLINE,
    DEAL_SET_FINANCING_DEADLINE,
];

pub const ROUTED_COMMAND_TYPES: &[&str] = &[
    DEAL_SET_STAGE_UNDER_CONTRACT,
    DEAL_SET_STAGE_CLOSED,
    DEAL_SET_CLOSING_DATE,
    DEAL_SET_INSPECTION_DEADLINE,
    DEAL_SET_FINANCING_DEADLINE,
    DEAL_SET_FINANCING_TYPE,
    DEAL_SET_APPRAISAL_REQUIRED,
    DEAL_SET_LENDER_CLEAR_TO_CLOSE,
    OFFER_ACCEPT,
    TASK_CREATE,
    TASK_COMPLETE,
    TASK_CANCEL,
];

pub fn is_routed(command_type: &str) -> bool {
    ROUTED_COMMAND_TYPES.contains(&command_type)
}

pub fn assert_command_nodes_routed(types: &[&str]) -> Vec<String> {
    let mut missing: Vec<String> = types
        .iter()
        .copied()
        .filter(|t| !is_routed(t))
        .map(str::to_string)
        .collect();
    missing.sort();
    missing.dedup();
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::xml::parse_re_supermodel;
    #[test]
    fn xml_command_nodes_are_routed() {
        let def = parse_re_supermodel().unwrap();
        let types: Vec<&str> = def
            .definition
            .nodes
            .values()
            .filter(|n| n.node_type == "command")
            .filter_map(|n| n.command_type.as_deref())
            .collect();
        assert!(assert_command_nodes_routed(&types).is_empty(), "{types:?}");
    }
}
