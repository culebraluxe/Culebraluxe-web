//! Port of model-turn-budget.ts.

pub const GENERATION_TURN_CAP_ENV: &str = "FORGE_MAX_MODEL_TURNS_PER_GENERATION";
pub const DEFAULT_MAX_GENERATION_TURNS: u32 = 10;

/// The stop code a capped generation carries into the record.
///
/// V1 called the same verdict `GENERATION_TURN_CAP` (`legacy/workflow_app/tests/forge-model-turn-budget.test.ts`
/// asserts that code). V2 names it **after what is being counted**, because the Cockpit renders the code beside
/// the count — `MODEL_TURN_CAP · stopped at 10 / 10 turns` — and because the environment variable that lifts the
/// cap is `FORGE_MAX_MODEL_TURNS_PER_GENERATION`. The V1 code is deliberately NOT kept as an alias: two names for
/// one stop is how a reader ends up matching on the wrong one.
pub const MODEL_TURN_CAP_CODE: &str = "MODEL_TURN_CAP";

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
                "{MODEL_TURN_CAP_CODE}: this generation has already dispatched {turns_used} turns (cap {cap}). \
                 Raise {GENERATION_TURN_CAP_ENV} to authorise a longer run, or split the work into another story."
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
