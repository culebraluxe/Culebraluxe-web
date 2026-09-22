use serde::{Deserialize, Serialize};

pub type OptionId = u8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inputs {
    pub property_name: String,
    pub purchase_price: f64,
    pub extra_spent: f64,
    pub contributory_to_date: f64,
    pub appraisal: f64,
    pub selling_cost_pct: f64,
    pub discount_rate: f64,
    pub tax_rate: f64,
    pub o1_on: bool,
    pub o1_months: f64,
    pub o1_salvage: f64,
    pub o1_low_delta: f64,
    pub o1_mid_delta: f64,
    pub o1_high_delta: f64,
    pub o1_p_low: f64,
    pub o1_p_mid: f64,
    pub o1_p_high: f64,
    pub o2_on: bool,
    pub o2_capex: f64,
    pub o2_months: f64,
    pub o2_recovery: f64,
    pub o2_salvage: f64,
    pub o2_low_delta: f64,
    pub o2_mid_delta: f64,
    pub o2_high_delta: f64,
    pub o2_p_low: f64,
    pub o2_p_mid: f64,
    pub o2_p_high: f64,
    pub o3_on: bool,
    pub o3_capex: f64,
    pub o3_months: f64,
    pub o3_noi: f64,
    pub o3_cap_rate: f64,
    pub o3_p_success: f64,
    pub o3_fail_salvage: f64,
    pub o3_low_delta: f64,
    pub o3_mid_delta: f64,
    pub o3_high_delta: f64,
    pub o3_p_low: f64,
    pub o3_p_mid: f64,
    pub o3_p_high: f64,
    pub o4_on: bool,
    pub o4_capex: f64,
    pub o4_months: f64,
    pub o4_asset_base: f64,
    pub o4_salvage: f64,
    pub o4_low_delta: f64,
    pub o4_mid_delta: f64,
    pub o4_high_delta: f64,
    pub o4_p_low: f64,
    pub o4_p_mid: f64,
    pub o4_p_high: f64,
    pub o5_on: bool,
    pub o5_share: f64,
    pub o5_months: f64,
    pub o5_period_cash: f64,
    pub o5_capex: f64,
    pub o5_low_delta: f64,
    pub o5_mid_delta: f64,
    pub o5_high_delta: f64,
    pub o5_p_low: f64,
    pub o5_p_mid: f64,
    pub o5_p_high: f64,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            property_name: "Island house + pods".into(),
            purchase_price: 325_000.0,
            extra_spent: 200_000.0,
            contributory_to_date: 86_000.0,
            appraisal: 425_000.0,
            selling_cost_pct: 0.07,
            discount_rate: 0.10,
            tax_rate: 0.15,
            o1_on: true,
            o1_months: 4.0,
            o1_salvage: 15_000.0,
            o1_low_delta: 0.0,
            o1_mid_delta: 0.10,
            o1_high_delta: 0.20,
            o1_p_low: 0.40,
            o1_p_mid: 0.40,
            o1_p_high: 0.20,
            o2_on: true,
            o2_capex: 80_000.0,
            o2_months: 12.0,
            o2_recovery: 0.65,
            o2_salvage: 12_000.0,
            o2_low_delta: -0.05,
            o2_mid_delta: 0.08,
            o2_high_delta: 0.18,
            o2_p_low: 0.30,
            o2_p_mid: 0.50,
            o2_p_high: 0.20,
            o3_on: true,
            o3_capex: 175_000.0,
            o3_months: 24.0,
            o3_noi: 90_000.0,
            o3_cap_rate: 0.09,
            o3_p_success: 0.55,
            o3_fail_salvage: 480_000.0,
            o3_low_delta: -0.15,
            o3_mid_delta: 0.0,
            o3_high_delta: 0.20,
            o3_p_low: 0.30,
            o3_p_mid: 0.50,
            o3_p_high: 0.20,
            o4_on: true,
            o4_capex: 25_000.0,
            o4_months: 8.0,
            o4_asset_base: 40_000.0,
            o4_salvage: 8_000.0,
            o4_low_delta: -0.05,
            o4_mid_delta: 0.05,
            o4_high_delta: 0.12,
            o4_p_low: 0.35,
            o4_p_mid: 0.45,
            o4_p_high: 0.20,
            o5_on: true,
            o5_share: 1.0,
            o5_months: 36.0,
            o5_period_cash: -36_000.0,
            o5_capex: 15_000.0,
            o5_low_delta: -0.05,
            o5_mid_delta: 0.08,
            o5_high_delta: 0.20,
            o5_p_low: 0.30,
            o5_p_mid: 0.50,
            o5_p_high: 0.20,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SellerStrategyState {
    pub inputs: Inputs,
    pub edit_all: bool,
    pub active_edit: Option<OptionId>,
    pub show_detail: bool,
}

impl Default for SellerStrategyState {
    fn default() -> Self {
        Self {
            inputs: Inputs::default(),
            edit_all: false,
            active_edit: None,
            show_detail: false,
        }
    }
}

impl SellerStrategyState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_option(&mut self, option: OptionId, enabled: bool) {
        match option {
            1 => self.inputs.o1_on = enabled,
            2 => self.inputs.o2_on = enabled,
            3 => self.inputs.o3_on = enabled,
            4 => self.inputs.o4_on = enabled,
            5 => self.inputs.o5_on = enabled,
            _ => {}
        }
        if !enabled && self.active_edit == Some(option) {
            self.active_edit = None;
        }
    }

    pub fn set_field(&mut self, key: &str, raw: &str, percent: bool) {
        if key == "propertyName" {
            self.inputs.property_name = raw.to_owned();
            return;
        }
        let mut value = raw.parse::<f64>().unwrap_or(0.0);
        if percent {
            value /= 100.0;
        }
        match key {
            "purchasePrice" => self.inputs.purchase_price = value,
            "extraSpent" => self.inputs.extra_spent = value,
            "contributoryToDate" => self.inputs.contributory_to_date = value,
            "appraisal" => self.inputs.appraisal = value,
            "sellingCostPct" => self.inputs.selling_cost_pct = value,
            "discountRate" => self.inputs.discount_rate = value,
            "taxRate" => self.inputs.tax_rate = value,
            "o1Months" => self.inputs.o1_months = value,
            "o1Salvage" => self.inputs.o1_salvage = value,
            "o1LowDelta" => self.inputs.o1_low_delta = value,
            "o1MidDelta" => self.inputs.o1_mid_delta = value,
            "o1HighDelta" => self.inputs.o1_high_delta = value,
            "o1PLow" => self.inputs.o1_p_low = value,
            "o1PMid" => self.inputs.o1_p_mid = value,
            "o1PHigh" => self.inputs.o1_p_high = value,
            "o2Capex" => self.inputs.o2_capex = value,
            "o2Months" => self.inputs.o2_months = value,
            "o2Recovery" => self.inputs.o2_recovery = value,
            "o2Salvage" => self.inputs.o2_salvage = value,
            "o2LowDelta" => self.inputs.o2_low_delta = value,
            "o2MidDelta" => self.inputs.o2_mid_delta = value,
            "o2HighDelta" => self.inputs.o2_high_delta = value,
            "o2PLow" => self.inputs.o2_p_low = value,
            "o2PMid" => self.inputs.o2_p_mid = value,
            "o2PHigh" => self.inputs.o2_p_high = value,
            "o3Capex" => self.inputs.o3_capex = value,
            "o3Months" => self.inputs.o3_months = value,
            "o3Noi" => self.inputs.o3_noi = value,
            "o3CapRate" => self.inputs.o3_cap_rate = value,
            "o3PSuccess" => self.inputs.o3_p_success = value,
            "o3FailSalvage" => self.inputs.o3_fail_salvage = value,
            "o3LowDelta" => self.inputs.o3_low_delta = value,
            "o3MidDelta" => self.inputs.o3_mid_delta = value,
            "o3HighDelta" => self.inputs.o3_high_delta = value,
            "o3PLow" => self.inputs.o3_p_low = value,
            "o3PMid" => self.inputs.o3_p_mid = value,
            "o3PHigh" => self.inputs.o3_p_high = value,
            "o4Capex" => self.inputs.o4_capex = value,
            "o4Months" => self.inputs.o4_months = value,
            "o4AssetBase" => self.inputs.o4_asset_base = value,
            "o4Salvage" => self.inputs.o4_salvage = value,
            "o4LowDelta" => self.inputs.o4_low_delta = value,
            "o4MidDelta" => self.inputs.o4_mid_delta = value,
            "o4HighDelta" => self.inputs.o4_high_delta = value,
            "o4PLow" => self.inputs.o4_p_low = value,
            "o4PMid" => self.inputs.o4_p_mid = value,
            "o4PHigh" => self.inputs.o4_p_high = value,
            "o5Share" => self.inputs.o5_share = value,
            "o5Months" => self.inputs.o5_months = value,
            "o5PeriodCash" => self.inputs.o5_period_cash = value,
            "o5Capex" => self.inputs.o5_capex = value,
            "o5LowDelta" => self.inputs.o5_low_delta = value,
            "o5MidDelta" => self.inputs.o5_mid_delta = value,
            "o5HighDelta" => self.inputs.o5_high_delta = value,
            "o5PLow" => self.inputs.o5_p_low = value,
            "o5PMid" => self.inputs.o5_p_mid = value,
            "o5PHigh" => self.inputs.o5_p_high = value,
            _ => {}
        }
    }
}

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

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.001
}

fn present_value(amount: f64, rate: f64, months: f64) -> f64 {
    amount / (1.0 + rate).powf(months / 12.0)
}

#[derive(Clone, Copy)]
struct BranchInput {
    id: &'static str,
    label: &'static str,
    option: OptionId,
    price: f64,
    p: f64,
    future_capex: f64,
    salvage: f64,
}

fn branch(input: BranchInput, inputs: &Inputs, months: f64, basis: f64) -> Branch {
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
    check_triple(inputs.o1_p_low + inputs.o1_p_mid + inputs.o1_p_high, "As-is", inputs.o1_on);
    check_triple(inputs.o2_p_low + inputs.o2_p_mid + inputs.o2_p_high, "Improve", inputs.o2_on);
    check_triple(inputs.o3_p_low + inputs.o3_p_mid + inputs.o3_p_high, "Join", inputs.o3_on);
    check_triple(inputs.o4_p_low + inputs.o4_p_mid + inputs.o4_p_high, "Fork", inputs.o4_on);
    check_triple(inputs.o5_p_low + inputs.o5_p_mid + inputs.o5_p_high, "Hold", inputs.o5_on);
    if inputs.o3_on && inputs.o3_cap_rate <= 0.0 {
        flags.push("Join cap rate must be greater than 0.".into());
    }
    if inputs.o3_on && !(0.0..=1.0).contains(&inputs.o3_p_success) {
        flags.push("Join P(success) must be 0-100%.".into());
    }
    if inputs.o5_on && (inputs.o5_share <= 0.0 || inputs.o5_share > 1.0) {
        flags.push("Hold/recap share must be between 0 and 100%.".into());
    }
    if ![inputs.o1_on, inputs.o2_on, inputs.o3_on, inputs.o4_on, inputs.o5_on]
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
    let pack = if inputs.o3_cap_rate > 0.0 { inputs.o3_noi / inputs.o3_cap_rate } else { 0.0 };
    let o3 = [
        pack * (1.0 + inputs.o3_low_delta),
        pack * (1.0 + inputs.o3_mid_delta),
        pack * (1.0 + inputs.o3_high_delta),
    ];
    let o4 = [
        inputs.appraisal * (1.0 + inputs.o4_low_delta) + inputs.o4_asset_base * (1.0 + inputs.o4_low_delta),
        inputs.appraisal * (1.0 + inputs.o4_mid_delta) + inputs.o4_asset_base * (1.0 + inputs.o4_mid_delta),
        inputs.appraisal * (1.0 + inputs.o4_high_delta) + inputs.o4_asset_base * (1.0 + inputs.o4_high_delta),
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

    push(BranchInput { id: "1L", label: "1-Low", option: 1, price: o1[0], p: inputs.o1_p_low, future_capex: 0.0, salvage: inputs.o1_salvage }, inputs.o1_on, inputs.o1_months);
    push(BranchInput { id: "1M", label: "1-Mid", option: 1, price: o1[1], p: inputs.o1_p_mid, future_capex: 0.0, salvage: inputs.o1_salvage }, inputs.o1_on, inputs.o1_months);
    push(BranchInput { id: "1H", label: "1-Ideal", option: 1, price: o1[2], p: inputs.o1_p_high, future_capex: 0.0, salvage: inputs.o1_salvage }, inputs.o1_on, inputs.o1_months);
    push(BranchInput { id: "2L", label: "2-Low", option: 2, price: o2[0], p: inputs.o2_p_low, future_capex: inputs.o2_capex, salvage: inputs.o2_salvage }, inputs.o2_on, inputs.o2_months);
    push(BranchInput { id: "2M", label: "2-Base", option: 2, price: o2[1], p: inputs.o2_p_mid, future_capex: inputs.o2_capex, salvage: inputs.o2_salvage }, inputs.o2_on, inputs.o2_months);
    push(BranchInput { id: "2H", label: "2-High", option: 2, price: o2[2], p: inputs.o2_p_high, future_capex: inputs.o2_capex, salvage: inputs.o2_salvage }, inputs.o2_on, inputs.o2_months);
    push(BranchInput { id: "3L", label: "3-Success Low", option: 3, price: o3[0], p: inputs.o3_p_success * inputs.o3_p_low, future_capex: inputs.o3_capex, salvage: 0.0 }, inputs.o3_on, inputs.o3_months);
    push(BranchInput { id: "3M", label: "3-Success Base", option: 3, price: o3[1], p: inputs.o3_p_success * inputs.o3_p_mid, future_capex: inputs.o3_capex, salvage: 0.0 }, inputs.o3_on, inputs.o3_months);
    push(BranchInput { id: "3H", label: "3-Success High", option: 3, price: o3[2], p: inputs.o3_p_success * inputs.o3_p_high, future_capex: inputs.o3_capex, salvage: 0.0 }, inputs.o3_on, inputs.o3_months);
    push(BranchInput { id: "3F", label: "3-Fail", option: 3, price: inputs.o3_fail_salvage, p: 1.0 - inputs.o3_p_success, future_capex: inputs.o3_capex, salvage: 0.0 }, inputs.o3_on, inputs.o3_months);
    push(BranchInput { id: "4L", label: "4-Low house+assets", option: 4, price: o4[0], p: inputs.o4_p_low, future_capex: inputs.o4_capex, salvage: inputs.o4_salvage }, inputs.o4_on, inputs.o4_months);
    push(BranchInput { id: "4M", label: "4-Mid house+assets", option: 4, price: o4[1], p: inputs.o4_p_mid, future_capex: inputs.o4_capex, salvage: inputs.o4_salvage }, inputs.o4_on, inputs.o4_months);
    push(BranchInput { id: "4H", label: "4-High house+assets", option: 4, price: o4[2], p: inputs.o4_p_high, future_capex: inputs.o4_capex, salvage: inputs.o4_salvage }, inputs.o4_on, inputs.o4_months);
    push(BranchInput { id: "5L", label: "5-Low terminal", option: 5, price: o5[0], p: inputs.o5_p_low, future_capex: inputs.o5_capex, salvage: inputs.o5_period_cash }, inputs.o5_on, inputs.o5_months);
    push(BranchInput { id: "5M", label: "5-Mid terminal", option: 5, price: o5[1], p: inputs.o5_p_mid, future_capex: inputs.o5_capex, salvage: inputs.o5_period_cash }, inputs.o5_on, inputs.o5_months);
    push(BranchInput { id: "5H", label: "5-High terminal", option: 5, price: o5[2], p: inputs.o5_p_high, future_capex: inputs.o5_capex, salvage: inputs.o5_period_cash }, inputs.o5_on, inputs.o5_months);

    let emv = |option: OptionId, pv_field: bool| -> f64 {
        branches
            .iter()
            .filter(|branch| branch.option == option)
            .map(|branch| branch.p * if pv_field { branch.pv } else { branch.after_tax })
            .sum()
    };

    let catalog = [
        (1, "1  Sell as-is", "As-is", inputs.o1_on, inputs.o1_months, 0.0),
        (2, "2  Improve then sell", "Improve", inputs.o2_on, inputs.o2_months, inputs.o2_capex),
        (3, "3  Join then sell package", "Join", inputs.o3_on, inputs.o3_months, inputs.o3_capex),
        (4, "4  Fork house and assets", "Fork", inputs.o4_on, inputs.o4_months, inputs.o4_capex),
        (5, "5  Hold or recap", "Hold", inputs.o5_on, inputs.o5_months, inputs.o5_capex - inputs.o5_period_cash),
    ];

    let mut raw: Vec<OptionScore> = catalog
        .into_iter()
        .map(|(option, name, short, on, months, future_cash)| OptionScore {
            option,
            name,
            short,
            on,
            months,
            future_cash,
            emv_undiscounted: if on { emv(option, false) } else { f64::NEG_INFINITY },
            emv_pv: if on { emv(option, true) } else { f64::NEG_INFINITY },
            vs_option1: 0.0,
            best: false,
        })
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
        score.vs_option1 = if score.on && o1_on { raw_pv - o1_pv } else { 0.0 };
        score.best = score.on && raw_pv == best_pv;
    }

    let mut ranked: Vec<OptionScore> = raw.iter().filter(|score| score.on).cloned().collect();
    ranked.sort_by(|a, b| b.emv_pv.total_cmp(&a.emv_pv));
    let winner = ranked.first().cloned().unwrap_or_else(|| raw[0].clone());
    let runner_up = ranked.get(1).cloned();

    ModelResult { basis, branches, scores: raw, winner, runner_up, flags }
}

pub fn rank_strategies(model: &ModelResult) -> Vec<OptionScore> {
    let mut ranked: Vec<OptionScore> = model.scores.iter().filter(|score| score.on).cloned().collect();
    ranked.sort_by(|a, b| b.emv_pv.total_cmp(&a.emv_pv));
    ranked
}

pub fn recommendation_rationale(model: &ModelResult) -> String {
    let winner = &model.winner;
    let enabled: Vec<&OptionScore> = model.scores.iter().filter(|score| score.on).collect();
    let shortest_months = enabled.iter().map(|score| score.months).fold(f64::INFINITY, f64::min);
    let strategy = match winner.option {
        1 => "Selling as-is requires no additional capital",
        2 => "The invested improvements are expected to lift proceeds above the as-is path",
        3 => "The stabilized package is expected to maximize value across the combined property + business interest",
        4 => "Separating the property and the other interests is expected to release more value than selling together",
        5 => "Holding or selling only a share is expected to preserve optionality while still returning value",
        _ => "This path leads on expected present value",
    };
    let mut parts = vec![format!(
        "{} currently provides the highest expected present value of {}",
        winner.short,
        money(winner.emv_pv)
    )];
    if winner.months <= shortest_months {
        parts.push("with the shortest path to liquidity".into());
    }
    if winner.future_cash > 0.0 {
        parts.push(format!("requiring {} of incremental capital", money(winner.future_cash)));
    }
    if let Some(runner_up) = &model.runner_up {
        let gap = winner.emv_pv - runner_up.emv_pv;
        if gap > 0.0 {
            parts.push(format!("beating {} by {}", runner_up.short, money(gap)));
        }
    }
    format!("{strategy} — {}.", parts.join(", "))
}

pub fn takeaways(model: &ModelResult, inputs: &Inputs) -> Vec<Takeaway> {
    let mut out = Vec::new();
    let enabled: Vec<&OptionScore> = model.scores.iter().filter(|score| score.on).collect();
    if !enabled.is_empty() {
        out.push(Takeaway {
            tone: TakeawayTone::Positive,
            text: format!("{} carries the highest expected PV at {}.", model.winner.short, money(model.winner.emv_pv)),
        });
        if let Some(fastest) = enabled.iter().min_by(|a, b| a.months.total_cmp(&b.months)) {
            if fastest.option != model.winner.option {
                out.push(Takeaway {
                    tone: TakeawayTone::Neutral,
                    text: format!("{} reaches liquidity soonest at {} months.", fastest.short, number(fastest.months)),
                });
            }
        }
        if let Some(heaviest) = enabled.iter().max_by(|a, b| a.future_cash.total_cmp(&b.future_cash)) {
            if heaviest.future_cash > 0.0 {
                out.push(Takeaway {
                    tone: TakeawayTone::Caution,
                    text: format!("{} requires the most incremental capital at {}.", heaviest.short, money(heaviest.future_cash)),
                });
            }
        }
        if let Some(longest) = enabled.iter().max_by(|a, b| a.months.total_cmp(&b.months)) {
            out.push(Takeaway {
                tone: TakeawayTone::Neutral,
                text: format!("{} has the longest horizon at {} months.", longest.short, number(longest.months)),
            });
        }
    }
    if let Some(worst) = model.branches.iter().min_by(|a, b| a.pv.total_cmp(&b.pv)) {
        out.push(Takeaway {
            tone: TakeawayTone::Caution,
            text: format!("The lowest single outcome is {} at {} PV.", worst.label, money(worst.pv)),
        });
    }
    if inputs.o3_on && inputs.o3_p_success < 1.0 {
        out.push(Takeaway {
            tone: TakeawayTone::Caution,
            text: format!("Join carries a {} failure branch at {} salvage.", pct(1.0 - inputs.o3_p_success), money(inputs.o3_fail_salvage)),
        });
    }
    for flag in model.flags.iter().take(2) {
        out.push(Takeaway { tone: TakeawayTone::Caution, text: flag.clone() });
    }
    out
}

pub fn money(value: f64) -> String {
    let rounded = value.abs().round() as i64;
    let digits = rounded.to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let grouped: String = grouped.chars().rev().collect();
    if value < 0.0 {
        format!("({}{})", "$", grouped)
    } else {
        format!("{}{}", "$", grouped)
    }
}

pub fn pct(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

pub fn compact_money(value: f64) -> String {
    let thousands = value / 1_000.0;
    if thousands < 0.0 {
        format!("-{}{:.0}k", "$", thousands.abs())
    } else {
        format!("{}{:.0}k", "$", thousands.abs())
    }
}

pub fn number(value: f64) -> String {
    if value.fract().abs() < 0.000_001 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_matches_the_live_strategy_shape() {
        let result = evaluate(&Inputs::default());
        assert_eq!(result.scores.len(), 5);
        assert_eq!(result.branches.len(), 16);
        assert!(result.flags.is_empty());
        assert!(result.winner.on);
    }

    #[test]
    fn probability_validation_survives_the_port() {
        let mut inputs = Inputs::default();
        inputs.o1_p_low = 0.9;
        let result = evaluate(&inputs);
        assert!(result.flags.iter().any(|flag| flag == "As-is probabilities must sum to 100%."));
    }

    #[test]
    fn percent_fields_are_normalized_by_the_reducer_state() {
        let mut state = SellerStrategyState::default();
        state.set_field("sellingCostPct", "7.5", true);
        assert!((state.inputs.selling_cost_pct - 0.075).abs() < 0.000_001);
    }
}
