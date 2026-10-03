//! Port of `workflow_app/forge/failure-classifier.ts`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgeFailureClass {
    MissingContext,
    BadImplementation,
    BadArchitecture,
    BadToolContract,
    EnvironmentFailure,
    MissingGuardrail,
    WeakTest,
    DependencyFailure,
    DeploymentFailure,
    Unknown,
}

impl ForgeFailureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingContext => "MISSING_CONTEXT",
            Self::BadImplementation => "BAD_IMPLEMENTATION",
            Self::BadArchitecture => "BAD_ARCHITECTURE",
            Self::BadToolContract => "BAD_TOOL_CONTRACT",
            Self::EnvironmentFailure => "ENVIRONMENT_FAILURE",
            Self::MissingGuardrail => "MISSING_GUARDRAIL",
            Self::WeakTest => "WEAK_TEST",
            Self::DependencyFailure => "DEPENDENCY_FAILURE",
            Self::DeploymentFailure => "DEPLOYMENT_FAILURE",
            Self::Unknown => "UNKNOWN",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "MISSING_CONTEXT" => Self::MissingContext,
            "BAD_IMPLEMENTATION" => Self::BadImplementation,
            "BAD_ARCHITECTURE" => Self::BadArchitecture,
            "BAD_TOOL_CONTRACT" => Self::BadToolContract,
            "ENVIRONMENT_FAILURE" => Self::EnvironmentFailure,
            "MISSING_GUARDRAIL" => Self::MissingGuardrail,
            "WEAK_TEST" => Self::WeakTest,
            "DEPENDENCY_FAILURE" => Self::DependencyFailure,
            "DEPLOYMENT_FAILURE" => Self::DeploymentFailure,
            "UNKNOWN" => Self::Unknown,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ForgeFailureClassification {
    pub class: ForgeFailureClass,
    pub reason: String,
    pub unknown: bool,
}

pub fn classify_failure(
    candidate: Option<&str>,
    observed: Option<&str>,
    detail: Option<&str>,
) -> ForgeFailureClassification {
    if let Some(c) = candidate.and_then(ForgeFailureClass::parse) {
        return ForgeFailureClassification {
            class: c,
            reason: detail.unwrap_or(c.as_str()).into(),
            unknown: false,
        };
    }
    let mapped = match observed {
        Some("deploy") => Some(ForgeFailureClass::DeploymentFailure),
        Some("env") => Some(ForgeFailureClass::EnvironmentFailure),
        Some("missing-context") => Some(ForgeFailureClass::MissingContext),
        Some("arch") => Some(ForgeFailureClass::BadArchitecture),
        Some("tool") => Some(ForgeFailureClass::BadToolContract),
        Some("dependency") => Some(ForgeFailureClass::DependencyFailure),
        Some("guardrail") => Some(ForgeFailureClass::MissingGuardrail),
        _ => None,
    };
    if let Some(cls) = mapped {
        return ForgeFailureClassification {
            class: cls,
            reason: detail.unwrap_or(cls.as_str()).into(),
            unknown: false,
        };
    }
    ForgeFailureClassification {
        class: ForgeFailureClass::Unknown,
        reason: detail.unwrap_or("unclassifiable failure").into(),
        unknown: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeFailureRouting {
    Repair { owner: &'static str, attempts: u32 },
    Hold { reason: String, attempts: u32 },
}

pub fn route_failure(
    class: ForgeFailureClass,
    attempts: u32,
    max_attempts: u32,
) -> ForgeFailureRouting {
    if attempts >= max_attempts {
        return ForgeFailureRouting::Hold {
            reason: format!(
                "retry budget exhausted ({}/{}) for {}",
                attempts,
                max_attempts,
                class.as_str()
            ),
            attempts,
        };
    }
    let owner = match class {
        ForgeFailureClass::MissingContext => Some("scout"),
        ForgeFailureClass::BadImplementation => Some("smith"),
        ForgeFailureClass::BadArchitecture => Some("architect"),
        ForgeFailureClass::BadToolContract | ForgeFailureClass::MissingGuardrail => Some("lead"),
        ForgeFailureClass::EnvironmentFailure | ForgeFailureClass::DeploymentFailure => {
            Some("dev_ops")
        }
        ForgeFailureClass::WeakTest => Some("qa"),
        ForgeFailureClass::DependencyFailure | ForgeFailureClass::Unknown => None,
    };
    match owner {
        Some(o) => ForgeFailureRouting::Repair { owner: o, attempts },
        None => ForgeFailureRouting::Hold {
            reason: format!("{} requires operator/Lead intervention", class.as_str()),
            attempts,
        },
    }
}

/// The class router's ceiling: the class to run with, once the durable budget has had its say.
///
/// WHY THIS EXISTS (captain, 2026-09-29). Forge has two ways to send a failure back for repair, and only one of
/// them was bounded. `qa_result` → `qa_failure_route` routes by the QA disposition through `route_qa_result`, which
/// is budgeted (3 repairs, 2 replans — see `qa_repair.rs`). `failure_classifier` → `failure_route` routes by
/// `failureClass` and had **no ceiling at all** — it is reached from a failed QA review, a failed publish, a failed
/// migration, a failed deploy and a failed smoke test, and each of those can fail again into the same classifier.
/// Production shows what that costs: `ENG-FORGE-V13` reached **15** repairs and `ENG-FORGE-OPENCODE-DOGFOOD-01`
/// **9**, both with `forge_last_qa_disposition` null — neither ever went through the budgeted door — and
/// `ENG-FORGE-TURN-VISIBILITY-01` sits at 11 in `In Progress`, a story the engine stopped holding.
///
/// The legacy engine held a failure class to a retry budget and escalated to a human at the ceiling; that is the
/// rule restored here. A class whose route has spent its budget is *demoted to `HOLD`* — demotion rather than
/// refusal, because the XML already owns the arm (`failure_route` routes `failureClass == 'HOLD'` to `hold`), so
/// the engine stops asking the same worker to try again and leaves the story where a person will see it. Never a
/// silent success and never another silent lap.
///
/// The counters are the durable ones on `storyboard_story`, written only by the completion ledger on entry to
/// `repair_smith` / `repair_architect` — one writer, one fact. `route_failure` makes the decision, so the two
/// routers cannot disagree about when a budget is spent.
pub fn budgeted_failure_class(
    class: ForgeFailureClass,
    repair_attempts: u32,
    replan_attempts: u32,
    max_repair: u32,
    max_replan: u32,
) -> Option<&'static str> {
    let (attempts, max) = match class {
        ForgeFailureClass::BadImplementation | ForgeFailureClass::WeakTest => {
            (repair_attempts, max_repair)
        }
        ForgeFailureClass::BadArchitecture => (replan_attempts, max_replan),
        // Everything else — an unknown cause, an environment, a migration, a publish, a deployment, a smoke test —
        // has no counter of its own, so it is charged against the whole repair cycle: what a reader would call
        // "how many times have we been round this loop already".
        _ => (repair_attempts.saturating_add(replan_attempts), max_repair),
    };
    match route_failure(class, attempts, max) {
        ForgeFailureRouting::Hold { .. } => Some("HOLD"),
        ForgeFailureRouting::Repair { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule: at the ceiling the class is demoted to HOLD, below it the class is left alone. Both halves
    /// matter — a router that demoted early would refuse good repairs, which is the failure mode this fix could
    /// itself introduce.
    #[test]
    fn a_class_over_its_budget_is_demoted_to_hold() {
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::BadImplementation, 3, 0, 3, 2),
            Some("HOLD")
        );
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::BadImplementation, 2, 0, 3, 2),
            None
        );
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::BadArchitecture, 0, 2, 3, 2),
            Some("HOLD")
        );
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::BadArchitecture, 0, 1, 3, 2),
            None
        );
    }

    /// The unbounded case this exists for: classes with no counter of their own still stop. `UNKNOWN_CAUSE`
    /// self-loops through `repair_scout` while `rootCauseKnown == false`, and the devops classes re-enter the
    /// classifier from six different nodes — none of which counted anything before this.
    #[test]
    fn classes_without_a_counter_of_their_own_are_charged_to_the_cycle() {
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::Unknown, 4, 1, 3, 2),
            Some("HOLD")
        );
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::EnvironmentFailure, 2, 1, 3, 2),
            Some("HOLD")
        );
        assert_eq!(
            budgeted_failure_class(ForgeFailureClass::EnvironmentFailure, 1, 0, 3, 2),
            None
        );
    }

    /// One authority: this and `route_failure` cannot answer differently about the ceiling.
    #[test]
    fn the_ceiling_agrees_with_route_failure() {
        for attempts in 0..5u32 {
            let routing = route_failure(ForgeFailureClass::BadImplementation, attempts, 3);
            let demoted =
                budgeted_failure_class(ForgeFailureClass::BadImplementation, attempts, 0, 3, 2);
            assert_eq!(
                demoted.is_some(),
                matches!(routing, ForgeFailureRouting::Hold { .. }),
                "attempts={attempts}"
            );
        }
    }
}
