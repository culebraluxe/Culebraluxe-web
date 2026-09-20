//! Port of model-turn-budget.ts.

pub const GENERATION_TURN_CAP_ENV: &str = "FORGE_MAX_MODEL_TURNS_PER_GENERATION";
pub const DEFAULT_MAX_GENERATION_TURNS: u32 = 10;

pub fn resolve_generation_turn_cap(raw: Option<&str>) -> u32 {
    let Some(v) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return DEFAULT_MAX_GENERATION_TURNS;
    };
    v.parse::<u32>()
        .ok()
        .map(|n| n.clamp(1, 100))
        .unwrap_or(DEFAULT_MAX_GENERATION_TURNS)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnBudgetVerdict {
    Allowed {
        turns_used: u32,
        cap: u32,
    },
    Refused {
        turns_used: u32,
        cap: u32,
        reason: String,
    },
}

pub fn assess_generation_turn_budget(turns_used: u32, cap: Option<u32>) -> TurnBudgetVerdict {
    let cap = cap.unwrap_or(DEFAULT_MAX_GENERATION_TURNS);
    if turns_used < cap {
        TurnBudgetVerdict::Allowed { turns_used, cap }
    } else {
        TurnBudgetVerdict::Refused {
            turns_used,
            cap,
            reason: format!(
                "MODEL TURN CAP: this generation has already dispatched {turns_used} turns (cap {cap})."
            ),
        }
    }
}

pub fn render_turn_budget_line(v: &TurnBudgetVerdict) -> String {
    let (used, cap, extra) = match v {
        TurnBudgetVerdict::Allowed { turns_used, cap } => (*turns_used, *cap, None),
        TurnBudgetVerdict::Refused {
            turns_used,
            cap,
            reason,
        } => (*turns_used, *cap, Some(reason.as_str())),
    };
    let remaining = cap.saturating_sub(used);
    let spent =
        format!("TURN BUDGET: {used} of {cap} model turns used; {remaining} before the cap");
    match extra {
        None => format!("{spent}."),
        Some(r) => format!("{spent}. {r}"),
    }
}
