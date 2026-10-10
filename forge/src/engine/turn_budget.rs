//! Port of model-turn-budget.ts.

use std::collections::HashSet;
use std::sync::Mutex;

pub const GENERATION_TURN_CAP_ENV: &str = "FORGE_MAX_MODEL_TURNS_PER_GENERATION";
pub const DEFAULT_MAX_GENERATION_TURNS: u32 = 10;
pub const WITHIN_STORY_CONCURRENCY_ENV: &str = "FORGE_WITHIN_STORY_CONCURRENCY";
pub const DEFAULT_WITHIN_STORY_CONCURRENCY: usize = 1;
pub const MAX_WITHIN_STORY_CONCURRENCY: usize = 4;

/// Concurrency for ready lanes within one workflow instance. This is separate
/// from resident story-worker concurrency and corrective model attempts.
pub fn resolve_within_story_concurrency(raw: Option<&str>) -> usize {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<usize>().ok())
        .map(|value| value.clamp(1, MAX_WITHIN_STORY_CONCURRENCY))
        .unwrap_or(DEFAULT_WITHIN_STORY_CONCURRENCY)
}

/// Service boundary for one generation's durable model-attempt ledger.
pub trait ModelAttemptControl: Send + Sync {
    fn reserve(&self, task_id: &str, role_attempt: u32) -> Result<ModelAttemptPermit, String>;
    fn finish(
        &self,
        permit: &ModelAttemptPermit,
        status: &str,
        detail: Option<&str>,
    ) -> Result<(), String>;
    fn generation_id(&self) -> &str;
    fn cap(&self) -> u32;
}

/// In-process attempt control for explicitly direct/non-durable executions.
/// Production durable composition replaces this with the database authority.
pub struct LocalModelAttemptControl {
    generation_id: String,
    cap: u32,
    state: Mutex<(u32, HashSet<String>)>,
}

impl LocalModelAttemptControl {
    pub fn new(cap: u32) -> Self {
        Self {
            generation_id: format!("direct-{}", uuid::Uuid::new_v4()),
            cap: cap.clamp(1, 100),
            state: Mutex::new((0, HashSet::new())),
        }
    }
}

impl ModelAttemptControl for LocalModelAttemptControl {
    fn reserve(&self, task_id: &str, role_attempt: u32) -> Result<ModelAttemptPermit, String> {
        let key = format!("{}:{task_id}:{role_attempt}", self.generation_id);
        let mut state = self
            .state
            .lock()
            .map_err(|_| "direct model attempt ledger is poisoned".to_string())?;
        if state.1.contains(&key) {
            return Err(format!(
                "{MODEL_TURN_CAP_CODE}: attempt {key} was already authorized; refusing duplicate launch"
            ));
        }
        if state.0 >= self.cap {
            return Err(format!(
                "{MODEL_TURN_CAP_CODE}: generation {} has used {} of {} model attempts; no allowance remains",
                self.generation_id, state.0, self.cap
            ));
        }
        state.0 += 1;
        state.1.insert(key.clone());
        Ok(ModelAttemptPermit {
            attempt_key: key,
            used: state.0,
            cap: self.cap,
        })
    }

    fn finish(
        &self,
        permit: &ModelAttemptPermit,
        _status: &str,
        _detail: Option<&str>,
    ) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "direct model attempt ledger is poisoned".to_string())?;
        if state.1.contains(&permit.attempt_key) {
            Ok(())
        } else {
            Err(format!(
                "unknown direct model attempt {}",
                permit.attempt_key
            ))
        }
    }

    fn generation_id(&self) -> &str {
        &self.generation_id
    }

    fn cap(&self) -> u32 {
        self.cap
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAttemptPermit {
    pub attempt_key: String,
    pub used: u32,
    pub cap: u32,
}

/// Production adapter. SQL remains in the database crate; this implementation
/// only transports the authorization capability through Forge's service layer.
pub struct DbModelAttemptControl {
    generation_id: String,
    story_id: String,
    story_run_id: String,
    cap: u32,
}

impl DbModelAttemptControl {
    pub fn initialize(
        generation_id: impl Into<String>,
        story_id: impl Into<String>,
        story_run_id: impl Into<String>,
        cap: u32,
    ) -> Result<Self, String> {
        let requested_cap = cap.clamp(1, 100);
        let generation_id = generation_id.into();
        let story_id = story_id.into();
        let story_run_id = story_run_id.into();
        let budget = crate::engine::vendor_session::with_shared(|db, rt| {
            rt.block_on(async {
                db::ForgeEngineDao::new(db.clone())
                    .ensure_model_generation_budget(&generation_id, &story_id, requested_cap as i32)
                    .await
                    .map_err(|error| error.to_string())
            })
        })??;
        Ok(Self {
            generation_id,
            story_id,
            story_run_id,
            cap: budget.cap as u32,
        })
    }
}

impl ModelAttemptControl for DbModelAttemptControl {
    fn reserve(&self, task_id: &str, role_attempt: u32) -> Result<ModelAttemptPermit, String> {
        crate::engine::vendor_session::with_shared(|db, rt| {
            rt.block_on(async {
                let reservation = db::ForgeEngineDao::new(db.clone())
                    .reserve_model_generation_attempt(
                        &self.generation_id,
                        &self.story_id,
                        &self.story_run_id,
                        self.cap as i32,
                        task_id,
                        role_attempt as i32,
                    )
                    .await.map_err(|error| error.to_string())?;
                if reservation.authorized {
                    eprintln!("forge-model-attempt generation={} task={} attempt={} status=authorized used={}/{}", self.generation_id, task_id, role_attempt, reservation.budget.used, reservation.budget.cap);
                    Ok(ModelAttemptPermit {
                        attempt_key: reservation.attempt_key,
                        used: reservation.budget.used as u32,
                        cap: reservation.budget.cap as u32,
                    })
                } else {
                    let reason = if reservation.duplicate {
                        format!("{MODEL_TURN_CAP_CODE}: attempt {} was already authorized; refusing duplicate launch ({} of {} used)", reservation.attempt_key, reservation.budget.used, reservation.budget.cap)
                    } else if reservation.budget.uncertain {
                        format!("{MODEL_TURN_CAP_CODE}: generation {} has uncertain legacy attempt history and is held for operator resolution", self.generation_id)
                    } else {
                        format!("{MODEL_TURN_CAP_CODE}: generation {} has used {} of {} model attempts; no allowance remains", self.generation_id, reservation.budget.used, reservation.budget.cap)
                    };
                    eprintln!(
                        "forge-model-attempt generation={} task={} attempt={} status=refused duplicate={} used={}/{} reason={}",
                        self.generation_id,
                        task_id,
                        role_attempt,
                        reservation.duplicate,
                        reservation.budget.used,
                        reservation.budget.cap,
                        if reservation.duplicate { "duplicate_authorization" } else { "generation_cap_exhausted" }
                    );
                    Err(reason)
                }
            })
        })?
    }

    fn finish(
        &self,
        permit: &ModelAttemptPermit,
        status: &str,
        detail: Option<&str>,
    ) -> Result<(), String> {
        crate::engine::vendor_session::with_shared(|db, rt| {
            rt.block_on(async {
                let changed = db::ForgeEngineDao::new(db.clone())
                    .mark_model_generation_attempt(
                        &self.generation_id,
                        &permit.attempt_key,
                        status,
                        detail,
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                if changed {
                    eprintln!(
                        "forge-model-attempt generation={} attempt={} status={status}",
                        self.generation_id, permit.attempt_key
                    );
                    Ok(())
                } else {
                    Err(format!(
                        "model attempt {} disappeared from durable ledger",
                        permit.attempt_key
                    ))
                }
            })
        })?
    }

    fn generation_id(&self) -> &str {
        &self.generation_id
    }
    fn cap(&self) -> u32 {
        self.cap
    }
}

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

#[cfg(test)]
mod concurrency_config_tests {
    use super::*;

    #[test]
    fn within_story_concurrency_defaults_to_one_and_clamps_explicit_values() {
        assert_eq!(resolve_within_story_concurrency(None), 1);
        assert_eq!(resolve_within_story_concurrency(Some(" ")), 1);
        assert_eq!(resolve_within_story_concurrency(Some("invalid")), 1);
        assert_eq!(resolve_within_story_concurrency(Some("2")), 2);
        assert_eq!(resolve_within_story_concurrency(Some("99")), 4);
    }

    #[test]
    fn local_attempt_control_counts_corrective_attempts_and_refuses_duplicates() {
        let control = LocalModelAttemptControl::new(2);
        assert_eq!(control.reserve("task-a", 0).unwrap().used, 1);
        assert!(control
            .reserve("task-a", 0)
            .unwrap_err()
            .contains("already authorized"));
        assert_eq!(control.reserve("task-a", 1).unwrap().used, 2);
        assert!(control
            .reserve("task-b", 0)
            .unwrap_err()
            .starts_with(MODEL_TURN_CAP_CODE));
    }
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
