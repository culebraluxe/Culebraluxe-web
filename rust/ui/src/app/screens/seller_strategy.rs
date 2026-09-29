//! CORE — Seller Strategy (`/portal/core/seller-strategy`): the sale-option calculator. Local only.

use yew::prelude::*;

use crate::app::cmd::Cmd;
use crate::app::screen::{Link, Screen, ScreenCtx};
use crate::seller_strategy::{
    compact_money, evaluate, money, number, pct, rank_strategies, recommendation_rationale,
    takeaways, Inputs, ModelResult, OptionId, OptionScore, SellerStrategyState, TakeawayTone,
};
mod definitions;
mod cockpit;
mod strategies;
#[allow(unused_imports)]
pub(super) use definitions::*;
#[allow(unused_imports)]
pub(super) use cockpit::*;
#[allow(unused_imports)]
pub(super) use strategies::*;


/// Every intent on the calculator. Nothing here reads or writes the server: the model is the assumptions, and the
/// evaluation is a pure function of them (`crate::seller_strategy`).
#[derive(Debug, PartialEq)]
pub enum Msg {
    SellerStrategyFieldChanged {
        key: String,
        raw: String,
        percent: bool,
    },
    SellerStrategyOptionToggled {
        option: u8,
        enabled: bool,
    },
    SellerStrategyEditAllToggled,
    SellerStrategyActiveEditChanged(Option<u8>),
    SellerStrategyDetailToggled,
    SellerStrategyReset,
}

pub struct SellerStrategy;

impl Screen for SellerStrategy {
    type Model = SellerStrategyState;
    type Msg = Msg;

    fn init(_ctx: &ScreenCtx) -> (SellerStrategyState, Cmd<Msg>) {
        (SellerStrategyState::default(), Cmd::none())
    }

    fn update(state: &mut SellerStrategyState, msg: Msg, _ctx: &ScreenCtx) -> Cmd<Msg> {
        match msg {
            Msg::SellerStrategyFieldChanged { key, raw, percent } => {
                state.set_field(&key, &raw, percent)
            }
            Msg::SellerStrategyOptionToggled { option, enabled } => {
                state.set_option(option, enabled)
            }
            Msg::SellerStrategyEditAllToggled => state.edit_all = !state.edit_all,
            Msg::SellerStrategyActiveEditChanged(option) => state.active_edit = option,
            Msg::SellerStrategyDetailToggled => state.show_detail = !state.show_detail,
            Msg::SellerStrategyReset => state.reset(),
        }
        Cmd::none()
    }

    fn view(state: &SellerStrategyState, _ctx: &ScreenCtx, link: &Link<Msg>) -> Html {
        cockpit(state, &link.callback(|msg: Msg| msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_calculator_is_local_and_its_intents_change_the_assumptions() {
        let ctx = ScreenCtx::default();
        let (mut state, cmd) = SellerStrategy::init(&ctx);
        assert!(cmd.into_requests().is_empty(), "no read");
        SellerStrategy::update(
            &mut state,
            Msg::SellerStrategyFieldChanged {
                key: "appraisal".into(),
                raw: "500000".into(),
                percent: false,
            },
            &ctx,
        );
        assert_eq!(state.inputs.appraisal, 500000.0);
        SellerStrategy::update(
            &mut state,
            Msg::SellerStrategyOptionToggled {
                option: 3,
                enabled: false,
            },
            &ctx,
        );
        assert!(!state.inputs.o3_on);
        SellerStrategy::update(&mut state, Msg::SellerStrategyReset, &ctx);
        assert_eq!(state, SellerStrategyState::default());
    }
}
