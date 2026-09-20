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
    Repair {
        owner: &'static str,
        attempts: u32,
    },
    Hold {
        reason: String,
        attempts: u32,
    },
}

pub fn route_failure(class: ForgeFailureClass, attempts: u32, max_attempts: u32) -> ForgeFailureRouting {
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
        ForgeFailureClass::EnvironmentFailure | ForgeFailureClass::DeploymentFailure => Some("dev_ops"),
        ForgeFailureClass::WeakTest => Some("qa"),
        ForgeFailureClass::DependencyFailure | ForgeFailureClass::Unknown => None,
    };
    match owner {
        Some(o) => ForgeFailureRouting::Repair {
            owner: o,
            attempts,
        },
        None => ForgeFailureRouting::Hold {
            reason: format!("{} requires operator/Lead intervention", class.as_str()),
            attempts,
        },
    }
}
