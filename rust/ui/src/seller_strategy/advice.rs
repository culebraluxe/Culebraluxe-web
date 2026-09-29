//! What the model says: ranking, the recommendation's rationale, takeaways, and money/percent formatting.

#[allow(unused_imports)]
use super::*;

pub fn rank_strategies(model: &ModelResult) -> Vec<OptionScore> {
    let mut ranked: Vec<OptionScore> = model
        .scores
        .iter()
        .filter(|score| score.on)
        .cloned()
        .collect();
    ranked.sort_by(|a, b| b.emv_pv.total_cmp(&a.emv_pv));
    ranked
}

pub fn recommendation_rationale(model: &ModelResult) -> String {
    let winner = &model.winner;
    let enabled: Vec<&OptionScore> = model.scores.iter().filter(|score| score.on).collect();
    let shortest_months = enabled
        .iter()
        .map(|score| score.months)
        .fold(f64::INFINITY, f64::min);
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
        parts.push(format!(
            "requiring {} of incremental capital",
            money(winner.future_cash)
        ));
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
            text: format!(
                "{} carries the highest expected PV at {}.",
                model.winner.short,
                money(model.winner.emv_pv)
            ),
        });
        if let Some(fastest) = enabled.iter().min_by(|a, b| a.months.total_cmp(&b.months)) {
            if fastest.option != model.winner.option {
                out.push(Takeaway {
                    tone: TakeawayTone::Neutral,
                    text: format!(
                        "{} reaches liquidity soonest at {} months.",
                        fastest.short,
                        number(fastest.months)
                    ),
                });
            }
        }
        if let Some(heaviest) = enabled
            .iter()
            .max_by(|a, b| a.future_cash.total_cmp(&b.future_cash))
        {
            if heaviest.future_cash > 0.0 {
                out.push(Takeaway {
                    tone: TakeawayTone::Caution,
                    text: format!(
                        "{} requires the most incremental capital at {}.",
                        heaviest.short,
                        money(heaviest.future_cash)
                    ),
                });
            }
        }
        if let Some(longest) = enabled.iter().max_by(|a, b| a.months.total_cmp(&b.months)) {
            out.push(Takeaway {
                tone: TakeawayTone::Neutral,
                text: format!(
                    "{} has the longest horizon at {} months.",
                    longest.short,
                    number(longest.months)
                ),
            });
        }
    }
    if let Some(worst) = model.branches.iter().min_by(|a, b| a.pv.total_cmp(&b.pv)) {
        out.push(Takeaway {
            tone: TakeawayTone::Caution,
            text: format!(
                "The lowest single outcome is {} at {} PV.",
                worst.label,
                money(worst.pv)
            ),
        });
    }
    if inputs.o3_on && inputs.o3_p_success < 1.0 {
        out.push(Takeaway {
            tone: TakeawayTone::Caution,
            text: format!(
                "Join carries a {} failure branch at {} salvage.",
                pct(1.0 - inputs.o3_p_success),
                money(inputs.o3_fail_salvage)
            ),
        });
    }
    for flag in model.flags.iter().take(2) {
        out.push(Takeaway {
            tone: TakeawayTone::Caution,
            text: flag.clone(),
        });
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
