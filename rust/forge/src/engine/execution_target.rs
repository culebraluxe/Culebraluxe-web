//! Port of workflow_app/forge/forge-execution-target.ts. Pure.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeEnvironmentError(pub String);

impl std::fmt::Display for ForgeEnvironmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for ForgeEnvironmentError {}

pub const FORGE_EXECUTION_ENVIRONMENT: &str = "PROD";

pub fn normalize_execution_target(target: Option<&str>) -> String {
    let raw = target.unwrap_or("").trim().to_ascii_uppercase();
    match raw.as_str() {
        "PRODUCTION" => "PROD".into(),
        "DEVELOPMENT" => "DEV".into(),
        other => other.to_string(),
    }
}

pub fn assert_forge_execution_target(
    target: Option<&str>,
    allow: Option<&str>,
) -> Result<&'static str, ForgeEnvironmentError> {
    let normalized = normalize_execution_target(target);
    if normalized == FORGE_EXECUTION_ENVIRONMENT {
        return Ok("PROD");
    }
    if let Some(allowed) = allow {
        if normalized == normalize_execution_target(Some(allowed)) {
            return Ok(if normalized == "TEST" { "TEST" } else { "PROD" });
        }
        if normalized == "TEST" && allowed.eq_ignore_ascii_case("TEST") {
            return Ok("TEST");
        }
    }
    Err(ForgeEnvironmentError(format!(
        "Forge runs against PROD only: resolved execution target was {:?}. Refusing to launch (fail closed).",
        target
    )))
}

pub fn resolve_forge_execution_target(env: &[(String, String)]) -> Result<&'static str, ForgeEnvironmentError> {
    let get = |k: &str| env.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    assert_forge_execution_target(get("EXECUTION_ENV").or_else(|| get("APP_ENV")), None)
}

fn describe_control_plane(env: &[(String, String)]) -> (&'static str, Option<String>) {
    let get = |k: &str| env.iter().find(|(a, _)| a.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
    if let Some(v) = get("APP_ENV") {
        let n = v.trim().to_ascii_lowercase();
        let target = match n.as_str() {
            "production" | "prod" => "prod",
            "preview" => "preview",
            "test" => "test",
            _ => "dev",
        };
        return (target, Some("APP_ENV".into()));
    }
    if let Some(v) = get("VERCEL_ENV") {
        let n = v.trim().to_ascii_lowercase();
        let target = match n.as_str() {
            "production" => "prod",
            "preview" => "preview",
            _ => "dev",
        };
        return (target, Some("VERCEL_ENV".into()));
    }
    ("undeclared", None)
}

pub fn assert_forge_lane_may_start(env: &[(String, String)]) -> Result<&'static str, ForgeEnvironmentError> {
    let target = resolve_forge_execution_target(env)?;
    let (control, declared_by) = describe_control_plane(env);
    if control != "prod" {
        return Err(ForgeEnvironmentError(format!(
            "Forge control plane must be PROD: the environment resolved to {:?} {}.",
            control,
            declared_by.map(|d| format!("(declared by {d})")).unwrap_or_else(|| "(undeclared)".into())
        )));
    }
    Ok(target)
}

pub fn env_pairs_from_process() -> Vec<(String, String)> {
    std::env::vars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_is_refused() {
        assert!(assert_forge_execution_target(None, None).is_err());
    }
    #[test]
    fn production_alias() {
        assert_eq!(assert_forge_execution_target(Some("production"), None).unwrap(), "PROD");
    }
    #[test]
    fn lane_needs_prod_control_plane() {
        let env = vec![
            ("EXECUTION_ENV".into(), "PROD".into()),
            ("APP_ENV".into(), "development".into()),
        ];
        assert!(assert_forge_lane_may_start(&env).is_err());
        let prod = vec![
            ("EXECUTION_ENV".into(), "PROD".into()),
            ("APP_ENV".into(), "production".into()),
        ];
        assert!(assert_forge_lane_may_start(&prod).is_ok());
    }
}
