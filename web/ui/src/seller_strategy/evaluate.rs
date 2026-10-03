//! The decision model: branches, present values, option scores and the evaluation of all five strategies.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub id: &'static str,
    pub label: &'static str,
    pub option: OptionId,
    pub price: f64,
    pub p: f64,
    pub selling_costs: f64,
    pub future_capex: f64,
    pub salvage: f64,
    pub pretax_net: f64,
    pub tax: f64,
    pub after_tax: f64,
    pub pv: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionScore {
    pub option: OptionId,
    pub name: &'static str,
    pub short: &'static str,
    pub on: bool,
    pub months: f64,
    pub future_cash: f64,
    pub emv_undiscounted: f64,
    pub emv_pv: f64,
    pub vs_option1: f64,
    pub best: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResult {
    pub basis: f64,
    pub branches: Vec<Branch>,
    pub scores: Vec<OptionScore>,
    pub winner: OptionScore,
    pub runner_up: Option<OptionScore>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeawayTone {
    Positive,
    Neutral,
    Caution,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Takeaway {
    pub tone: TakeawayTone,
    pub text: String,
}

pub(super) fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.001
}

pub(super) fn present_value(amount: f64, rate: f64, months: f64) -> f64 {
    amount / (1.0 + rate).powf(months / 12.0)
}

#[derive(Clone, Copy)]
pub(super) struct BranchInput {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) option: OptionId,
    pub(super) price: f64,
    pub(super) p: f64,
    pub(super) future_capex: f64,
    pub(super) salvage: f64,
}

pub(super) fn branch(input: BranchInput, inputs: &Inputs, months: f64, basis: f64) -> Branch {
    let selling_costs = input.price * inputs.selling_cost_pct;
    let pretax_net = input.price - selling_costs - input.future_capex + input.salvage;
    let tax = (input.price - basis).max(0.0) * inputs.tax_rate;
    let after_tax = pretax_net - tax;
    Branch {
        id: input.id,
        label: input.label,
        option: input.option,
        price: input.price,
        p: input.p,
        selling_costs,
        future_capex: input.future_capex,
        salvage: input.salvage,
        pretax_net,
        tax,
        after_tax,
        pv: present_value(after_tax, inputs.discount_rate, months),
    }
}

pub fn evaluate(inputs: &Inputs) -> ModelResult {
    let basis = inputs.purchase_price + inputs.contributory_to_date;
    let mut flags = Vec::new();
    let mut check_triple = |sum: f64, label: &str, on: bool| {
        if on && !near(sum, 1.0) {
            flags.push(format!("{label} probabilities must sum to 100%."));
        }
    };
    check_triple(
        inputs.o1_p_low + inputs.o1_p_mid + inputs.o1_p_high,
        "As-is",
        inputs.o1_on,
    );
    check_triple(
        inputs.o2_p_low + inputs.o2_p_mid + inputs.o2_p_high,
        "Improve",
        inputs.o2_on,
    );
    check_triple(
        inputs.o3_p_low + inputs.o3_p_mid + inputs.o3_p_high,
        "Join",
        inputs.o3_on,
    );
    check_triple(
        inputs.o4_p_low + inputs.o4_p_mid + inputs.o4_p_high,
        "Fork",
        inputs.o4_on,
    );
    check_triple(
        inputs.o5_p_low + inputs.o5_p_mid + inputs.o5_p_high,
        "Hold",
        inputs.o5_on,
    );
    if inputs.o3_on && inputs.o3_cap_rate <= 0.0 {
        flags.push("Join cap rate must be greater than 0.".into());
    }
    if inputs.o3_on && !(0.0..=1.0).contains(&inputs.o3_p_success) {
        flags.push("Join P(success) must be 0-100%.".into());
    }
    if inputs.o5_on && (inputs.o5_share <= 0.0 || inputs.o5_share > 1.0) {
        flags.push("Hold/recap share must be between 0 and 100%.".into());
    }
    if ![
        inputs.o1_on,
        inputs.o2_on,
        inputs.o3_on,
        inputs.o4_on,
        inputs.o5_on,
    ]
    .into_iter()
    .any(|on| on)
    {
        flags.push("Turn on at least one path.".into());
    }

    let o1 = [
        inputs.appraisal * (1.0 + inputs.o1_low_delta),
        inputs.appraisal * (1.0 + inputs.o1_mid_delta),
        inputs.appraisal * (1.0 + inputs.o1_high_delta),
    ];
    let improved = inputs.appraisal + inputs.o2_capex * inputs.o2_recovery;
    let o2 = [
        improved * (1.0 + inputs.o2_low_delta),
        improved * (1.0 + inputs.o2_mid_delta),
        improved * (1.0 + inputs.o2_high_delta),
    ];
    let pack = if inputs.o3_cap_rate > 0.0 {
        inputs.o3_noi / inputs.o3_cap_rate
    } else {
        0.0
    };
    let o3 = [
        pack * (1.0 + inputs.o3_low_delta),
        pack * (1.0 + inputs.o3_mid_delta),
        pack * (1.0 + inputs.o3_high_delta),
    ];
    let o4 = [
        inputs.appraisal * (1.0 + inputs.o4_low_delta)
            + inputs.o4_asset_base * (1.0 + inputs.o4_low_delta),
        inputs.appraisal * (1.0 + inputs.o4_mid_delta)
            + inputs.o4_asset_base * (1.0 + inputs.o4_mid_delta),
        inputs.appraisal * (1.0 + inputs.o4_high_delta)
            + inputs.o4_asset_base * (1.0 + inputs.o4_high_delta),
    ];
    let share = inputs.o5_share.clamp(0.0, 1.0);
    let o5 = [
        inputs.appraisal * (1.0 + inputs.o5_low_delta) * share,
        inputs.appraisal * (1.0 + inputs.o5_mid_delta) * share,
        inputs.appraisal * (1.0 + inputs.o5_high_delta) * share,
    ];

    let mut branches = Vec::new();
    let mut push = |input: BranchInput, on: bool, months: f64| {
        if on {
            branches.push(branch(input, inputs, months, basis));
        }
    };

    push(
        BranchInput {
            id: "1L",
            label: "1-Low",
            option: 1,
            price: o1[0],
            p: inputs.o1_p_low,
            future_capex: 0.0,
            salvage: inputs.o1_salvage,
        },
        inputs.o1_on,
        inputs.o1_months,
    );
    push(
        BranchInput {
            id: "1M",
            label: "1-Mid",
            option: 1,
            price: o1[1],
            p: inputs.o1_p_mid,
            future_capex: 0.0,
            salvage: inputs.o1_salvage,
        },
        inputs.o1_on,
        inputs.o1_months,
    );
    push(
        BranchInput {
            id: "1H",
            label: "1-Ideal",
            option: 1,
            price: o1[2],
            p: inputs.o1_p_high,
            future_capex: 0.0,
            salvage: inputs.o1_salvage,
        },
        inputs.o1_on,
        inputs.o1_months,
    );
    push(
        BranchInput {
            id: "2L",
            label: "2-Low",
            option: 2,
            price: o2[0],
            p: inputs.o2_p_low,
            future_capex: inputs.o2_capex,
            salvage: inputs.o2_salvage,
        },
        inputs.o2_on,
        inputs.o2_months,
    );
    push(
        BranchInput {
            id: "2M",
            label: "2-Base",
            option: 2,
            price: o2[1],
            p: inputs.o2_p_mid,
            future_capex: inputs.o2_capex,
            salvage: inputs.o2_salvage,
        },
        inputs.o2_on,
        inputs.o2_months,
    );
    push(
        BranchInput {
            id: "2H",
            label: "2-High",
            option: 2,
            price: o2[2],
            p: inputs.o2_p_high,
            future_capex: inputs.o2_capex,
            salvage: inputs.o2_salvage,
        },
        inputs.o2_on,
        inputs.o2_months,
    );
    push(
        BranchInput {
            id: "3L",
            label: "3-Success Low",
            option: 3,
            price: o3[0],
            p: inputs.o3_p_success * inputs.o3_p_low,
            future_capex: inputs.o3_capex,
            salvage: 0.0,
        },
        inputs.o3_on,
        inputs.o3_months,
    );
    push(
        BranchInput {
            id: "3M",
            label: "3-Success Base",
            option: 3,
            price: o3[1],
            p: inputs.o3_p_success * inputs.o3_p_mid,
            future_capex: inputs.o3_capex,
            salvage: 0.0,
        },
        inputs.o3_on,
        inputs.o3_months,
    );
    push(
        BranchInput {
            id: "3H",
            label: "3-Success High",
            option: 3,
            price: o3[2],
            p: inputs.o3_p_success * inputs.o3_p_high,
            future_capex: inputs.o3_capex,
            salvage: 0.0,
        },
        inputs.o3_on,
        inputs.o3_months,
    );
    push(
        BranchInput {
            id: "3F",
            label: "3-Fail",
            option: 3,
            price: inputs.o3_fail_salvage,
            p: 1.0 - inputs.o3_p_success,
            future_capex: inputs.o3_capex,
            salvage: 0.0,
        },
        inputs.o3_on,
        inputs.o3_months,
    );
    push(
        BranchInput {
            id: "4L",
            label: "4-Low house+assets",
            option: 4,
            price: o4[0],
            p: inputs.o4_p_low,
            future_capex: inputs.o4_capex,
            salvage: inputs.o4_salvage,
        },
        inputs.o4_on,
        inputs.o4_months,
    );
    push(
        BranchInput {
            id: "4M",
            label: "4-Mid house+assets",
            option: 4,
            price: o4[1],
            p: inputs.o4_p_mid,
            future_capex: inputs.o4_capex,
            salvage: inputs.o4_salvage,
        },
        inputs.o4_on,
        inputs.o4_months,
    );
    push(
        BranchInput {
            id: "4H",
            label: "4-High house+assets",
            option: 4,
            price: o4[2],
            p: inputs.o4_p_high,
            future_capex: inputs.o4_capex,
            salvage: inputs.o4_salvage,
        },
        inputs.o4_on,
        inputs.o4_months,
    );
    push(
        BranchInput {
            id: "5L",
            label: "5-Low terminal",
            option: 5,
            price: o5[0],
            p: inputs.o5_p_low,
            future_capex: inputs.o5_capex,
            salvage: inputs.o5_period_cash,
        },
        inputs.o5_on,
        inputs.o5_months,
    );
    push(
        BranchInput {
            id: "5M",
            label: "5-Mid terminal",
            option: 5,
            price: o5[1],
            p: inputs.o5_p_mid,
            future_capex: inputs.o5_capex,
            salvage: inputs.o5_period_cash,
        },
        inputs.o5_on,
        inputs.o5_months,
    );
    push(
        BranchInput {
            id: "5H",
            label: "5-High terminal",
            option: 5,
            price: o5[2],
            p: inputs.o5_p_high,
            future_capex: inputs.o5_capex,
            salvage: inputs.o5_period_cash,
        },
        inputs.o5_on,
        inputs.o5_months,
    );

    let emv = |option: OptionId, pv_field: bool| -> f64 {
        branches
            .iter()
            .filter(|branch| branch.option == option)
            .map(|branch| {
                branch.p
                    * if pv_field {
                        branch.pv
                    } else {
                        branch.after_tax
                    }
            })
            .sum()
    };

    let catalog = [
        (
            1,
            "1  Sell as-is",
            "As-is",
            inputs.o1_on,
            inputs.o1_months,
            0.0,
        ),
        (
            2,
            "2  Improve then sell",
            "Improve",
            inputs.o2_on,
            inputs.o2_months,
            inputs.o2_capex,
        ),
        (
            3,
            "3  Join then sell package",
            "Join",
            inputs.o3_on,
            inputs.o3_months,
            inputs.o3_capex,
        ),
        (
            4,
            "4  Fork house and assets",
            "Fork",
            inputs.o4_on,
            inputs.o4_months,
            inputs.o4_capex,
        ),
        (
            5,
            "5  Hold or recap",
            "Hold",
            inputs.o5_on,
            inputs.o5_months,
            inputs.o5_capex - inputs.o5_period_cash,
        ),
    ];

    let mut raw: Vec<OptionScore> = catalog
        .into_iter()
        .map(
            |(option, name, short, on, months, future_cash)| OptionScore {
                option,
                name,
                short,
                on,
                months,
                future_cash,
                emv_undiscounted: if on {
                    emv(option, false)
                } else {
                    f64::NEG_INFINITY
                },
                emv_pv: if on {
                    emv(option, true)
                } else {
                    f64::NEG_INFINITY
                },
                vs_option1: 0.0,
                best: false,
            },
        )
        .collect();

    let best_pv = raw
        .iter()
        .filter(|score| score.on)
        .map(|score| score.emv_pv)
        .fold(f64::NEG_INFINITY, f64::max);
    let o1_pv = if raw[0].on { raw[0].emv_pv } else { 0.0 };
    let o1_on = raw[0].on;

    for score in &mut raw {
        let raw_pv = score.emv_pv;
        if !score.on {
            score.emv_undiscounted = 0.0;
            score.emv_pv = 0.0;
        }
        score.vs_option1 = if score.on && o1_on {
            raw_pv - o1_pv
        } else {
            0.0
        };
        score.best = score.on && raw_pv == best_pv;
    }

    let mut ranked: Vec<OptionScore> = raw.iter().filter(|score| score.on).cloned().collect();
    ranked.sort_by(|a, b| b.emv_pv.total_cmp(&a.emv_pv));
    let winner = ranked.first().cloned().unwrap_or_else(|| raw[0].clone());
    let runner_up = ranked.get(1).cloned();

    ModelResult {
        basis,
        branches,
        scores: raw,
        winner,
        runner_up,
        flags,
    }
}
