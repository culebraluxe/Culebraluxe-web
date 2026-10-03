//! Each strategy's card, the active strategy, the branch table, tips and the decision tree.

#[allow(unused_imports)]
use super::*;

pub(super) fn strategy_card(
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
            let enabled = crate::app::exec::checked(&event);
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

pub(super) fn active_strategy_panel(
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

pub(super) fn branch_table(model: &ModelResult) -> Html {
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

pub(super) fn tips(option: OptionId) -> &'static [(&'static str, &'static str)] {
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

pub(super) fn strategy_color(option: OptionId) -> (&'static str, &'static str) {
    match option {
        1 => ("#1B365D", "#D6EAF8"),
        2 => ("#0F6E6B", "#D5F5E3"),
        3 => ("#B85C38", "#F5CBA7"),
        4 => ("#6C3483", "#E8DAEF"),
        _ => ("#7D6608", "#FCF3CF"),
    }
}

pub(super) fn decision_tree(model: &ModelResult) -> Html {
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
