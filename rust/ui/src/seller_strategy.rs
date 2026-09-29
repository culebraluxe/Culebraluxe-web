use serde::{Deserialize, Serialize};
mod advice;
mod evaluate;
#[allow(unused_imports)]
pub use advice::*;
#[allow(unused_imports)]
pub use evaluate::*;

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
        assert!(result
            .flags
            .iter()
            .any(|flag| flag == "As-is probabilities must sum to 100%."));
    }

    #[test]
    fn percent_fields_are_normalized_by_the_reducer_state() {
        let mut state = SellerStrategyState::default();
        state.set_field("sellingCostPct", "7.5", true);
        assert!((state.inputs.selling_cost_pct - 0.075).abs() < 0.000_001);
    }
}
