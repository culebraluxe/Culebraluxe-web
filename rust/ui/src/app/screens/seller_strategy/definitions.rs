//! Seller Strategy's inputs and the five strategies: field kinds, field and strategy definitions.

#[allow(unused_imports)]
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldKind {
    Text,
    Money,
    Months,
    Percent,
}

#[derive(Clone, Copy)]
pub(super) struct FieldDef {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) kind: FieldKind,
}

#[derive(Clone, Copy)]
pub(super) struct StrategyDef {
    pub(super) id: OptionId,
    pub(super) title: &'static str,
    pub(super) short: &'static str,
    pub(super) blurb: &'static str,
    pub(super) accent: &'static str,
    pub(super) fields: &'static [FieldDef],
}

pub(super) const PARENT_KEY: &[FieldDef] = &[
    FieldDef {
        key: "propertyName",
        label: "Name",
        kind: FieldKind::Text,
    },
    FieldDef {
        key: "appraisal",
        label: "Appraisal",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "sellingCostPct",
        label: "Selling costs %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "discountRate",
        label: "Discount %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "taxRate",
        label: "Tax placeholder %",
        kind: FieldKind::Percent,
    },
];

pub(super) const PARENT_BASIS: &[FieldDef] = &[
    FieldDef {
        key: "purchasePrice",
        label: "Purchase / basis",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "extraSpent",
        label: "Extra spent",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "contributoryToDate",
        label: "Contributory extra",
        kind: FieldKind::Money,
    },
];

pub(super) const O1_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "o1Months",
        label: "Months",
        kind: FieldKind::Months,
    },
    FieldDef {
        key: "o1Salvage",
        label: "Salvage",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o1LowDelta",
        label: "Low vs appraisal %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o1MidDelta",
        label: "Mid vs appraisal %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o1HighDelta",
        label: "Ideal vs appraisal %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o1PLow",
        label: "P low %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o1PMid",
        label: "P mid %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o1PHigh",
        label: "P ideal %",
        kind: FieldKind::Percent,
    },
];

pub(super) const O2_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "o2Capex",
        label: "Capex",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o2Months",
        label: "Months",
        kind: FieldKind::Months,
    },
    FieldDef {
        key: "o2Recovery",
        label: "Recovery %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o2Salvage",
        label: "Salvage",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o2LowDelta",
        label: "Low vs improved %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o2MidDelta",
        label: "Base vs improved %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o2HighDelta",
        label: "High vs improved %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o2PLow",
        label: "P low %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o2PMid",
        label: "P base %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o2PHigh",
        label: "P high %",
        kind: FieldKind::Percent,
    },
];

pub(super) const O3_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "o3Capex",
        label: "Launch cash",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o3Months",
        label: "Months",
        kind: FieldKind::Months,
    },
    FieldDef {
        key: "o3Noi",
        label: "Stabilized NOI",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o3CapRate",
        label: "Cap rate %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o3PSuccess",
        label: "P stabilize %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o3FailSalvage",
        label: "Fail salvage price",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o3LowDelta",
        label: "Low vs NOI/cap %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o3MidDelta",
        label: "Base vs NOI/cap %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o3HighDelta",
        label: "High vs NOI/cap %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o3PLow",
        label: "P low | success %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o3PMid",
        label: "P base | success %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o3PHigh",
        label: "P high | success %",
        kind: FieldKind::Percent,
    },
];

pub(super) const O4_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "o4Capex",
        label: "Split cost",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o4Months",
        label: "Months",
        kind: FieldKind::Months,
    },
    FieldDef {
        key: "o4AssetBase",
        label: "Asset proceeds base",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o4Salvage",
        label: "Leftover salvage",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o4LowDelta",
        label: "Low %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o4MidDelta",
        label: "Mid %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o4HighDelta",
        label: "High %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o4PLow",
        label: "P low %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o4PMid",
        label: "P mid %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o4PHigh",
        label: "P high %",
        kind: FieldKind::Percent,
    },
];

pub(super) const O5_FIELDS: &[FieldDef] = &[
    FieldDef {
        key: "o5Share",
        label: "Share sold %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o5Months",
        label: "Months",
        kind: FieldKind::Months,
    },
    FieldDef {
        key: "o5PeriodCash",
        label: "Income/(carry) over period",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o5Capex",
        label: "Keep-up capex",
        kind: FieldKind::Money,
    },
    FieldDef {
        key: "o5LowDelta",
        label: "Low vs appraisal %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o5MidDelta",
        label: "Mid vs appraisal %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o5HighDelta",
        label: "High vs appraisal %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o5PLow",
        label: "P low %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o5PMid",
        label: "P mid %",
        kind: FieldKind::Percent,
    },
    FieldDef {
        key: "o5PHigh",
        label: "P high %",
        kind: FieldKind::Percent,
    },
];

pub(super) const STRATEGIES: &[StrategyDef] = &[
    StrategyDef {
        id: 1,
        title: "Sell As-Is",
        short: "As-is",
        blurb: "Same house. Extra kit is salvage.",
        accent: "#1B365D",
        fields: O1_FIELDS,
    },
    StrategyDef {
        id: 2,
        title: "Improve then Sell",
        short: "Improve",
        blurb: "Same product, more brick.",
        accent: "#0F6E6B",
        fields: O2_FIELDS,
    },
    StrategyDef {
        id: 3,
        title: "Join",
        short: "Join",
        blurb: "One ticket: house + business.",
        accent: "#B85C38",
        fields: O3_FIELDS,
    },
    StrategyDef {
        id: 4,
        title: "Fork",
        short: "Fork",
        blurb: "House and assets separate.",
        accent: "#6C3483",
        fields: O4_FIELDS,
    },
    StrategyDef {
        id: 5,
        title: "Hold",
        short: "Hold",
        blurb: "Keep or sell only a share.",
        accent: "#7D6608",
        fields: O5_FIELDS,
    },
];
