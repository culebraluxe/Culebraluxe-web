use forge::engine::qa_plan::ApprovedAssayPlan;
use std::fs;

fn main() {
    if let Err(error) = run() {
        eprintln!("forge-assay-plan-hash: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| "usage: forge-assay-plan-hash <plan.json>".to_string())?;
    if std::env::args().nth(2).is_some() {
        return Err("expected one plan file path".into());
    }
    let source = fs::read_to_string(&path).map_err(|error| format!("read {path}: {error}"))?;
    if source.len() > 1024 * 1024 {
        return Err("plan exceeds the 1 MiB input limit".into());
    }
    let plan: ApprovedAssayPlan =
        serde_json::from_str(&source).map_err(|error| format!("decode typed plan: {error}"))?;
    if plan.schema_version != forge::engine::qa_plan::ASSAY_PLAN_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema version {}; expected {}",
            plan.schema_version,
            forge::engine::qa_plan::ASSAY_PLAN_SCHEMA_VERSION
        ));
    }
    let packet_commands = plan
        .commands
        .iter()
        .map(|command| command.command.clone())
        .collect::<Vec<_>>();
    plan.validate("operator-plan-validation", &packet_commands)
        .map_err(|errors| format!("plan validation failed: {}", errors.join("; ")))?;
    let identity = plan.identity()?;
    println!("plan_validation=passed");
    println!("plan_id={}", identity.plan_id);
    println!("plan_version={}", identity.plan_version);
    println!("schema_version={}", identity.schema_version);
    println!("approved_hash={}", identity.hash);
    println!("packet_binding=checked when the run snapshot is created");
    Ok(())
}
