use yew::prelude::*;

use crate::model::Msg;
use crate::seller_strategy::{
    compact_money, evaluate, money, number, pct, rank_strategies, recommendation_rationale,
    takeaways, Inputs, ModelResult, OptionId, OptionScore, TakeawayTone,
};
use crate::yew_views::portal_shell::PortalShell;

#[derive(Properties, PartialEq)]
pub struct SellerStrategyProps {
    pub model: crate::model::Model,
    pub on_msg: Callback<Msg>,
}

pub struct SellerStrategy;

impl Component for SellerStrategy {
    type Message = ();
    type Properties = SellerStrategyProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let screen =
            crate::model::screen("seller-strategy").expect("seller strategy screen exists");
        html! {
            <PortalShell screen={screen} model={props.model.clone()} on_msg={props.on_msg.clone()}>
                { cockpit(&props.model, &props.on_msg) }
            </PortalShell>
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Text,
    Money,
    Months,
    Percent,
}

#[derive(Clone, Copy)]
struct FieldDef {
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
}

#[derive(Clone, Copy)]
struct StrategyDef {
    id: OptionId,
    title: &'static str,
    short: &'static str,
    blurb: &'static str,
    accent: &'static str,
    fields: &'static [FieldDef],
}

const PARENT_KEY: &[FieldDef] = &[
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

const PARENT_BASIS: &[FieldDef] = &[
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

const O1_FIELDS: &[FieldDef] = &[
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

const O2_FIELDS: &[FieldDef] = &[
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

const O3_FIELDS: &[FieldDef] = &[
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

const O4_FIELDS: &[FieldDef] = &[
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

const O5_FIELDS: &[FieldDef] = &[
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

const STRATEGIES: &[StrategyDef] = &[
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

fn cockpit(model: &crate::model::Model, on_msg: &Callback<Msg>) -> Html {
    let state = &model.seller_strategy;
    let result = evaluate(&state.inputs);
    let ranking = rank_strategies(&result);
    let rationale = recommendation_rationale(&result);
    let takeaways = takeaways(&result, &state.inputs);
    let max_pv = ranking.first().map(|score| score.emv_pv).unwrap_or(1.0);
    let pdf_payload = serde_json::json!({
        "inputs": &state.inputs,
        "model": &result,
    })
    .to_string();

    let reset = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SellerStrategyReset))
    };
    let edit_all = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SellerStrategyEditAllToggled))
    };
    let detail_toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SellerStrategyDetailToggled))
    };

    html! {
        <div class="space-y-5">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div>
                    <p class="text-xs uppercase tracking-[0.22em] text-[var(--portal-gold-muted)]">
                        {"Core · Strategic disposition"}
                    </p>
                    <h1 class="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">
                        {"Seller Strategy"}
                    </h1>
                    <p class="mt-1 text-sm text-[var(--portal-muted)]">
                        {"Strategic disposition analysis for sellers — change any assumption and the model recalculates live."}
                    </p>
                </div>
                <div class="flex items-center gap-2">
                    <button
                        type="button"
                        onclick={reset}
                        class="rounded-full border border-[var(--portal-border)] bg-white/50 px-4 py-1.5 text-xs font-medium text-[var(--portal-navy)] transition hover:bg-white/70"
                    >
                        {"Reset"}
                    </button>
                    <form method="post" action="/api/portal/seller-strategy/pdf">
                        <input type="hidden" name="payload" value={pdf_payload} />
                        <button
                            type="submit"
                            class="rounded-full bg-[var(--portal-gold)] px-4 py-1.5 text-xs font-semibold text-[var(--portal-navy)] shadow-sm transition hover:bg-[var(--portal-gold-soft)]"
                        >
                            {"Download PDF"}
                        </button>
                    </form>
                </div>
            </div>

            if !result.flags.is_empty() {
                <div class="rounded-2xl border border-[var(--portal-danger)]/40 bg-[var(--portal-danger)]/10 px-4 py-3 text-sm text-[var(--portal-danger)]">
                    { for result.flags.iter().map(|flag| html! { <p>{ flag.clone() }</p> }) }
                </div>
            }

            <section class="portal-glass-panel portal-glass-panel-lifted overflow-hidden rounded-2xl p-1">
                <div style="display:grid;grid-template-columns:minmax(0,2fr) minmax(280px,1fr);gap:4px;">
                    <div class="portal-glass-panel portal-glass-panel-feature min-w-0 rounded-xl p-5">
                        <div style="display:grid;grid-template-columns:minmax(0,2fr) minmax(240px,1fr);gap:24px;">
                            <div class="min-w-0">
                                <p class="text-xs uppercase tracking-[0.24em] text-[var(--portal-feature-eyebrow)]">{"Recommended"}</p>
                                <h2 class="mt-2 font-serif text-3xl font-light text-white">{ result.winner.name }</h2>
                                <p class="mt-2 text-lg text-white">
                                    { money(result.winner.emv_pv) }
                                    <span class="text-sm text-[var(--portal-feature-muted)]">{" expected PV"}</span>
                                    <span class="mx-2 text-[var(--portal-feature-muted)]">{"·"}</span>
                                    { number(result.winner.months) }
                                    <span class="text-sm text-[var(--portal-feature-muted)]">{" months to liquidity"}</span>
                                </p>
                                <div class="mt-3 rounded-xl border border-white/12 bg-white/5 p-3">
                                    <p class="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-feature-eyebrow)]">{"Why this strategy?"}</p>
                                    <p class="mt-1 text-sm leading-5 text-[var(--portal-feature-muted)]">{ rationale }</p>
                                </div>
                            </div>
                            <div class="min-w-0 rounded-xl border border-white/12 bg-white/5 p-4">
                                <p class="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-feature-eyebrow)]">{"Expected PV Ranking"}</p>
                                <div class="mt-3 space-y-2.5">
                                    { for ranking.iter().map(|score| ranking_row(score, max_pv)) }
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="portal-glass-panel portal-glass-panel-soft min-w-0 rounded-xl p-5">
                        <div class="mb-3 flex items-end justify-between gap-3">
                            <div>
                                <p class="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">{"Shared facts"}</p>
                                <h2 class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Live Assumptions"}</h2>
                            </div>
                            <p class="text-right text-xs text-[var(--portal-muted)]">
                                { format!("Sunk {} · basis {}", money(state.inputs.purchase_price + state.inputs.extra_spent), money(result.basis)) }
                            </p>
                        </div>
                        <div style="display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;">
                            { for PARENT_KEY.iter().map(|field| {
                                if field.key == "propertyName" {
                                    html! { <div style="grid-column:1 / -1;">{ field_view(&state.inputs, *field, on_msg) }</div> }
                                } else {
                                    field_view(&state.inputs, *field, on_msg)
                                }
                            }) }
                        </div>
                        <button
                            type="button"
                            onclick={edit_all}
                            class="mt-3 text-xs font-medium text-[var(--portal-navy)] underline decoration-[var(--portal-gold)] underline-offset-2 hover:text-[var(--portal-navy-soft)]"
                        >
                            { if state.edit_all { "Hide basis & sunk-cost fields" } else { "Edit basis & sunk-cost fields" } }
                        </button>
                        if state.edit_all {
                            <div class="mt-3" style="display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;">
                                { for PARENT_BASIS.iter().map(|field| field_view(&state.inputs, *field, on_msg)) }
                            </div>
                        }
                    </div>
                </div>
            </section>

            <section style="display:grid;grid-template-columns:repeat(5,minmax(0,1fr));gap:16px;">
                { for STRATEGIES.iter().map(|strategy| strategy_card(strategy, &result, state.active_edit, on_msg)) }
            </section>

            if let Some(active_id) = state.active_edit {
                { active_strategy_panel(active_id, &state.inputs, &result, on_msg) }
            }

            <section style="display:grid;grid-template-columns:minmax(0,3fr) minmax(280px,1fr);gap:16px;align-items:start;">
                <div class="portal-glass-panel portal-glass-panel-lifted min-w-0 rounded-2xl p-5">
                    <div class="mb-3 flex items-end justify-between gap-3">
                        <div>
                            <p class="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">{"Hero visual"}</p>
                            <h2 class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Decision Map"}</h2>
                        </div>
                        <p class="text-xs text-[var(--portal-muted)]">
                            {"Live decision tree with probabilities, outcomes, and present values"}
                        </p>
                    </div>
                    <div class="overflow-x-auto rounded-xl">
                        <div class="mx-auto w-full min-w-[620px] max-w-[700px]">
                            { decision_tree(&result) }
                        </div>
                    </div>
                </div>

                <aside class="portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted min-w-0 rounded-2xl p-5">
                    <p class="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">{"Read-out"}</p>
                    <h2 class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Key Takeaways"}</h2>
                    <ul class="mt-3 space-y-2.5">
                        { for takeaways.iter().map(|item| {
                            let tone = match item.tone {
                                TakeawayTone::Positive => "var(--portal-success)",
                                TakeawayTone::Caution => "var(--portal-danger)",
                                TakeawayTone::Neutral => "var(--portal-blue-gray)",
                            };
                            html! {
                                <li class="flex items-start gap-2 text-sm leading-5">
                                    <span class="mt-1.5 h-1.5 w-1.5 flex-none rounded-full" style={format!("background-color:{tone}")}></span>
                                    <span class="text-[var(--portal-text)]">{ item.text.clone() }</span>
                                </li>
                            }
                        }) }
                    </ul>
                </aside>
            </section>

            <section class="portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted rounded-2xl p-5">
                <div class="mb-3 flex items-end justify-between gap-3">
                    <div>
                        <p class="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">{"Evidence"}</p>
                        <h2 class="font-serif text-lg font-light text-[var(--portal-navy)]">{"Analysis Detail"}</h2>
                    </div>
                    <button
                        type="button"
                        onclick={detail_toggle}
                        class="rounded-full border border-[var(--portal-border)] bg-white/50 px-3 py-1.5 text-[11px] font-medium text-[var(--portal-navy)] transition hover:bg-white/70"
                    >
                        { if state.show_detail { "Collapse" } else { "Show All Branches" } }
                    </button>
                </div>
                if state.show_detail {
                    { branch_table(&result) }
                } else {
                    <p class="text-sm text-[var(--portal-muted)]">
                        { format!(
                            "{} branches across {} enabled paths · winner {} at {} PV. Open Show All Branches for the full probability, price, after-tax and discounted-PV breakdown.",
                            result.branches.len(),
                            ranking.len(),
                            result.winner.short,
                            money(result.winner.emv_pv)
                        ) }
                    </p>
                }
            </section>
        </div>
    }
}

fn ranking_row(score: &OptionScore, max_pv: f64) -> Html {
    let width = if max_pv > 0.0 {
        ((score.emv_pv / max_pv) * 100.0).max(4.0)
    } else {
        0.0
    };
    let fill = if score.best {
        "var(--portal-gold)"
    } else {
        "rgba(255,255,255,0.5)"
    };
    html! {
        <div class="flex items-center gap-2">
            <span class="w-14 flex-none text-[11px] font-medium text-white">{ score.short }</span>
            <div class="h-2 flex-1 overflow-hidden rounded-full bg-white/12">
                <div class="h-full rounded-full" style={format!("width:{width:.1}%;background-color:{fill}")}></div>
            </div>
            <span class="w-20 flex-none text-right text-[11px] text-[var(--portal-feature-muted)]">
                { money(score.emv_pv) }
            </span>
        </div>
    }
}

fn field_view(inputs: &Inputs, field: FieldDef, on_msg: &Callback<Msg>) -> Html {
    let value = field_value(inputs, field);
    let key = field.key.to_owned();
    let percent = field.kind == FieldKind::Percent;
    let callback = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let raw = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            on_msg.emit(Msg::SellerStrategyFieldChanged {
                key: key.clone(),
                raw,
                percent,
            });
        })
    };

    html! {
        <label class="block">
            <span class="mb-0.5 block text-[10px] uppercase tracking-wide text-[var(--portal-muted)]">
                { field.label }
            </span>
            <input
                class="w-full rounded-md border border-[var(--portal-border)] bg-white/60 px-2 py-1.5 text-sm text-[var(--portal-text)] outline-none transition focus:border-[var(--portal-gold)]"
                value={value}
                oninput={callback}
            />
        </label>
    }
}

fn field_value(inputs: &Inputs, field: FieldDef) -> String {
    let Ok(value) = serde_json::to_value(inputs) else {
        return String::new();
    };
    let Some(value) = value.get(field.key) else {
        return String::new();
    };
    if field.kind == FieldKind::Text {
        return value.as_str().unwrap_or_default().to_owned();
    }
    let number = value.as_f64().unwrap_or(0.0);
    let number = if field.kind == FieldKind::Percent {
        number * 100.0
    } else {
        number
    };
    crate::seller_strategy::number((number * 10.0).round() / 10.0)
}

fn strategy_card(
    strategy: &StrategyDef,
    model: &ModelResult,
    active_edit: Option<OptionId>,
    on_msg: &Callback<Msg>,
) -> Html {
    let score = model
        .scores
        .iter()
        .find(|score| score.option == strategy.id);
    let Some(score) = score else {
        return html! {};
    };
    let option = strategy.id;
    let toggle = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: Event| {
            let enabled = event
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .checked();
            on_msg.emit(Msg::SellerStrategyOptionToggled { option, enabled });
        })
    };
    let edit = {
        let on_msg = on_msg.clone();
        let next = if active_edit == Some(strategy.id) {
            None
        } else {
            Some(strategy.id)
        };
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SellerStrategyActiveEditChanged(next)))
    };
    let opacity = if score.on { "" } else { "opacity:0.55;" };
    let value_class = if score.best && score.on {
        "text-xl font-semibold text-[var(--portal-gold-muted)]"
    } else {
        "text-xl font-semibold text-[var(--portal-navy)]"
    };

    html! {
        <article
            class="portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted min-w-0 rounded-2xl p-4 transition"
            style={format!("border-top-color:{};border-top-width:2px;{opacity}", strategy.accent)}
        >
            <div class="mb-2 flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <h3 class="text-sm font-semibold text-[var(--portal-navy)]">{ strategy.title }</h3>
                    <p class="text-[11px] text-[var(--portal-muted)]">{ strategy.blurb }</p>
                </div>
                <label class="flex items-center gap-1 text-[10px] uppercase tracking-wide text-[var(--portal-muted)]">
                    <input type="checkbox" checked={score.on} onchange={toggle} />
                    {"On"}
                </label>
            </div>
            <p class={value_class}>{ if score.on { money(score.emv_pv) } else { "—".into() } }</p>
            <p class="mt-1 text-[11px] text-[var(--portal-muted)]">
                { format!(
                    "{} mo · future {}{}",
                    number(score.months),
                    money(score.future_cash),
                    if score.best && score.on { " · BEST" } else { "" }
                ) }
            </p>
            <button
                type="button"
                onclick={edit}
                disabled={!score.on}
                class="mt-3 w-full rounded-full border border-[var(--portal-border)] bg-white/50 px-3 py-1.5 text-[11px] font-medium text-[var(--portal-navy)] transition hover:bg-white/70 disabled:cursor-not-allowed disabled:opacity-50"
            >
                { if active_edit == Some(strategy.id) { "Close Assumptions" } else { "Edit Assumptions" } }
            </button>
        </article>
    }
}

fn active_strategy_panel(
    active_id: OptionId,
    inputs: &Inputs,
    model: &ModelResult,
    on_msg: &Callback<Msg>,
) -> Html {
    let Some(strategy) = STRATEGIES.iter().find(|strategy| strategy.id == active_id) else {
        return html! {};
    };
    let score = model.scores.iter().find(|score| score.option == active_id);
    let collapse = {
        let on_msg = on_msg.clone();
        Callback::from(move |_: MouseEvent| on_msg.emit(Msg::SellerStrategyActiveEditChanged(None)))
    };
    html! {
        <section
            class="portal-glass-panel portal-glass-panel-soft portal-glass-panel-lifted rounded-2xl p-5"
            style={format!("border-top-color:{};border-top-width:2px;", strategy.accent)}
        >
            <div class="mb-4 flex flex-wrap items-start justify-between gap-3">
                <div>
                    <p class="text-[10px] uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">
                        { format!("{} ASSUMPTIONS", strategy.short.to_uppercase()) }
                    </p>
                    <h3 class="mt-1 font-serif text-lg font-light text-[var(--portal-navy)]">{ strategy.title }</h3>
                    <p class="text-xs text-[var(--portal-muted)]">
                        { format!("{} expected PV · changes apply immediately", money(score.map(|score| score.emv_pv).unwrap_or(0.0))) }
                    </p>
                </div>
                <button
                    type="button"
                    onclick={collapse}
                    class="rounded-full border border-[var(--portal-border)] bg-white/50 px-3 py-1.5 text-[11px] font-medium text-[var(--portal-navy)] transition hover:bg-white/70"
                >
                    {"Collapse"}
                </button>
            </div>
            <div style="display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;">
                { for strategy.fields.iter().map(|field| field_view(inputs, *field, on_msg)) }
            </div>
        </section>
    }
}

fn branch_table(model: &ModelResult) -> Html {
    html! {
        <div class="overflow-x-auto rounded-xl">
            <table class="w-full min-w-[640px] text-left text-sm">
                <thead class="text-[10px] uppercase tracking-wide text-[var(--portal-muted)]">
                    <tr>
                        <th class="pb-2">{"Branch"}</th>
                        <th>{"P"}</th>
                        <th>{"Price"}</th>
                        <th>{"After-tax proceeds"}</th>
                        <th>{"PV"}</th>
                    </tr>
                </thead>
                <tbody>
                    { for model.branches.iter().map(|branch| html! {
                        <tr class="border-t border-[var(--portal-border)]/50">
                            <td class="py-1.5">{ branch.label }</td>
                            <td>{ pct(branch.p) }</td>
                            <td>{ money(branch.price) }</td>
                            <td>{ money(branch.after_tax) }</td>
                            <td class="font-semibold text-[var(--portal-navy)]">{ money(branch.pv) }</td>
                        </tr>
                    }) }
                </tbody>
            </table>
        </div>
    }
}

fn tips(option: OptionId) -> &'static [(&'static str, &'static str)] {
    match option {
        1 => &[("1L", "LOW"), ("1M", "MID"), ("1H", "IDEAL")],
        2 => &[("2L", "LOW"), ("2M", "BASE"), ("2H", "HIGH")],
        3 => &[
            ("3L", "LOW"),
            ("3M", "BASE"),
            ("3H", "HIGH"),
            ("3F", "FAIL"),
        ],
        4 => &[("4L", "LOW"), ("4M", "MID"), ("4H", "HIGH")],
        _ => &[("5L", "LOW"), ("5M", "MID"), ("5H", "HIGH")],
    }
}

fn strategy_color(option: OptionId) -> (&'static str, &'static str) {
    match option {
        1 => ("#1B365D", "#D6EAF8"),
        2 => ("#0F6E6B", "#D5F5E3"),
        3 => ("#B85C38", "#F5CBA7"),
        4 => ("#6C3483", "#E8DAEF"),
        _ => ("#7D6608", "#FCF3CF"),
    }
}

fn decision_tree(model: &ModelResult) -> Html {
    let enabled: Vec<&OptionScore> = model.scores.iter().filter(|score| score.on).collect();
    if enabled.is_empty() {
        return html! { <p class="text-sm text-[#5D6D7E]">{"Turn on at least one path to see the tree."}</p> };
    }

    let row_h = 52.0;
    let path_gap = 22.0;
    let mut y = 16.0;
    let mut layout = Vec::new();
    for score in enabled {
        let count = tips(score.option).len() as f64;
        let height = (count * row_h).max(72.0);
        let y0 = y;
        y += height + path_gap;
        layout.push((score, y0, height, y0 + height / 2.0));
    }
    let height = (y + 8.0).max(280.0);
    let decide_y = height / 2.0 - 22.0;

    html! {
        <svg
            viewBox={format!("0 0 764 {height}")}
            width="764"
            height={height.to_string()}
            role="img"
            aria-label="Decision tree"
            style="width:100%;height:auto;min-height:280px;display:block;"
        >
            <rect x="16" y={decide_y.to_string()} width="88" height="44" rx="4" fill="#1B365D" />
            <text x="60" y={(decide_y + 18.0).to_string()} text-anchor="middle" fill="#fff" font-size="10" font-weight="700">{"DECIDE"}</text>
            <text x="60" y={(decide_y + 32.0).to_string()} text-anchor="middle" fill="#fff" font-size="10" font-weight="700">{"now"}</text>
            { for layout.into_iter().map(|(score, y0, _height, mid_y)| {
                let (stroke, fill) = strategy_color(score.option);
                let option_tips = tips(score.option);
                html! {
                    <g>
                        <line x1="104" y1={(decide_y + 22.0).to_string()} x2="150" y2={mid_y.to_string()} stroke={stroke} stroke-width="1.6" />
                        <rect x="150" y={(mid_y - 20.0).to_string()} width="132" height="40" rx="4" fill={fill} stroke={stroke} />
                        <text x="216" y={(mid_y - 4.0).to_string()} text-anchor="middle" fill={stroke} font-size="11" font-weight="700">{ score.short.to_uppercase() }</text>
                        <text x="216" y={(mid_y + 12.0).to_string()} text-anchor="middle" fill="#5D6D7E" font-size="9" font-weight="600">{ format!("{} mo", number(score.months)) }</text>
                        <line x1="282" y1={mid_y.to_string()} x2="302" y2={mid_y.to_string()} stroke={stroke} stroke-width="1.4" />
                        <circle cx="318" cy={mid_y.to_string()} r="14" fill={fill} stroke={stroke} />
                        <text x="318" y={(mid_y + 3.0).to_string()} text-anchor="middle" fill="#5D6D7E" font-size="9" font-weight="600">{"P"}</text>
                        { for option_tips.iter().enumerate().filter_map(|(index, (id, name))| {
                            let branch = model.branches.iter().find(|branch| branch.id == *id)?;
                            let yy = y0 + index as f64 * row_h + 4.0;
                            let box_mid = yy + 20.0;
                            Some(html! {
                                <g>
                                    <line x1="332" y1={mid_y.to_string()} x2="400" y2={box_mid.to_string()} stroke={stroke} stroke-width="1.2" />
                                    <text x="366" y={((mid_y + box_mid) / 2.0 - 4.0).to_string()} text-anchor="middle" fill={stroke} font-size="10" font-weight="700">
                                        { pct(branch.p) }
                                    </text>
                                    <rect x="400" y={yy.to_string()} width="210" height="40" rx="4" fill="#fff" stroke={stroke} />
                                    <text x="408" y={(yy + 16.0).to_string()} fill={stroke} font-size="11" font-weight="700">{ *name }</text>
                                    <text x="408" y={(yy + 32.0).to_string()} fill="#2C3E50" font-size="10" font-weight="600">
                                        { format!("{} · PV {}", compact_money(branch.price), compact_money(branch.pv)) }
                                    </text>
                                </g>
                            })
                        }) }
                        <rect x="630" y={(mid_y - 13.0).to_string()} width="118" height="26" rx="13" fill={stroke} />
                        <text x="689" y={(mid_y + 4.0).to_string()} text-anchor="middle" fill="#fff" font-size="10" font-weight="700">
                            { format!("{}{}", compact_money(score.emv_pv), if score.best { " BEST" } else { "" }) }
                        </text>
                    </g>
                }
            }) }
        </svg>
    }
}
