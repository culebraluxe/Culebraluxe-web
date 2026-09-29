//! The strategy cockpit: the inputs, the ranking and each field.

#[allow(unused_imports)]
use super::*;

pub(super) fn cockpit(state: &SellerStrategyState, on_msg: &Callback<Msg>) -> Html {
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
                    <form method="post" action={crate::app::api::links::SELLER_STRATEGY_PDF}>
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

pub(super) fn ranking_row(score: &OptionScore, max_pv: f64) -> Html {
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

pub(super) fn field_view(inputs: &Inputs, field: FieldDef, on_msg: &Callback<Msg>) -> Html {
    let value = field_value(inputs, field);
    let key = field.key.to_owned();
    let percent = field.kind == FieldKind::Percent;
    let callback = {
        let on_msg = on_msg.clone();
        Callback::from(move |event: InputEvent| {
            let raw = crate::app::exec::input_value(&event);
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

pub(super) fn field_value(inputs: &Inputs, field: FieldDef) -> String {
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
