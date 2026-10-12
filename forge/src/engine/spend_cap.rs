//! Port of forge-spend-cap.ts.
//!
//! NOTE ON PROVENANCE: unlike `turn_budget`, the TypeScript this was ported from is NOT in this repository (the
//! only budget test under `legacy/workflow_app/tests/` is `forge-model-turn-budget.test.ts`), and V1 had no stop
//! code for it at all — its spend figure travelled as a report field (`spendUsd` / `spendCapUsd` in
//! `legacy/workflow_app/tests/forge-v12.test.ts`). The environment variable and the stop code below are therefore
//! V2-owned names, chosen to sit beside `FORGE_MAX_MODEL_TURNS_PER_GENERATION` / `MODEL_TURN_CAP`, and the reason
//! is the same in both cases: a cap a reader cannot find is a cap nobody can lift.

/// The cap, in US dollars, on what ONE model turn may spend before Forge stops it.
pub const SPEND_CAP_ENV: &str = "FORGE_SPEND_CAP_USD";

/// The stop code a turn carries into the record when it is cut off by the cap.
pub const BUDGET_EXHAUSTED_CODE: &str = "BUDGET_EXHAUSTED";

use crate::engine::harness::HarnessUsage;
use workflow::{Result, WorkflowError};

pub fn parse_forge_spend_cap_usd(raw: Option<&str>) -> Option<f64> {
    let v = raw?.trim();
    if v.is_empty() {
        return None;
    }
    let n: f64 = v.parse().ok()?;
    if n.is_finite() && n >= 0.0 {
        Some(n)
    } else {
        None
    }
}

pub fn forge_spend_should_hold(spend_usd: Option<f64>, cap_usd: Option<f64>) -> bool {
    match (spend_usd, cap_usd) {
        (Some(s), Some(c)) => s > c,
        _ => false,
    }
}

/// The line a stop carries: what was spent, what the cap was, and which variable lifts it.
///
/// A cap with no stated figure is indistinguishable from a crash, and an unmeasured spend must never be rendered
/// as a number — `None` reads as "not measured", because a fabricated `$0.000000` next to `BUDGET_EXHAUSTED` would
/// be the one number in the record that is a lie.
pub fn render_spend_cap_line(spend_usd: Option<f64>, cap_usd: f64) -> String {
    match spend_usd {
        Some(spend) => format!(
            "{BUDGET_EXHAUSTED_CODE}: {spend:.6} USD measured against a {cap_usd:.6} USD cap ({SPEND_CAP_ENV})."
        ),
        None => format!(
            "{BUDGET_EXHAUSTED_CODE}: spend is unmeasured against a {cap_usd:.6} USD cap ({SPEND_CAP_ENV})."
        ),
    }
}

/// Post-hoc spend-cap verdict: the enforcement that always holds (WO-205).
///
/// `maestro-cli send` returns its cost in the final envelope, AFTER the turn ran, so this check
/// runs after the envelope parse in the Maestro `run_role` and holds the turn when the measured
/// spend passes the cap. Missing usage is UNKNOWN spend, never $0 (`forge_spend_should_hold`
/// holds only on a measured figure over the cap), and no configured cap holds nothing. The
/// failure carries BUDGET_EXHAUSTED with the measured line, the same vocabulary as the OpenCode
/// live-kill path.
pub fn check_post_hoc_spend_cap(usage: Option<&HarnessUsage>, cap: Option<f64>) -> Result<()> {
    if let Some(cap) = cap {
        let spend = usage.map(|turn| turn.cost_usd);
        if forge_spend_should_hold(spend, Some(cap)) {
            return Err(WorkflowError::generic(render_spend_cap_line(spend, cap)));
        }
    }
    Ok(())
}
