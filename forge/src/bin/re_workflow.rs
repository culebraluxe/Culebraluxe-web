//! Residential transaction host. Replaces workflow_app/runtime.ts engine door.

use forge::engine::re_runtime::{
    complete_engine_task, complete_workflow_task, reclaim_stale_jobs,
    reclaim_stale_jobs_for_instance, reconcile_closing_timer, reconcile_deadline_timer,
    reconcile_workflows, reset_dev_workflows, run_due_jobs, start_residential_transaction,
    workflow_status,
};
use std::env;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let out = match cmd {
        "start-deal" => {
            let id = flag(&args, "--id").expect("--id deal id");
            start_residential_transaction("deal", &id)
                .map(|r| format!("instance={} started={}", r.instance_id, r.started))
        }
        "start-contract" => {
            let id = flag(&args, "--id").expect("--id contract id");
            start_residential_transaction("contract", &id)
                .map(|r| format!("instance={} started={}", r.instance_id, r.started))
        }
        "timer" => {
            let instance = flag(&args, "--instance").expect("--instance");
            let node = flag(&args, "--node").unwrap_or_else(|| "closing_date_timer".into());
            let date = flag(&args, "--date");
            if node == "closing_date_timer" {
                reconcile_closing_timer(&instance, date.as_deref()).map(|a| a.to_string())
            } else {
                reconcile_deadline_timer(&instance, &node, date.as_deref()).map(|a| a.to_string())
            }
        }
        "complete-task" => {
            let app_task = flag(&args, "--application-task").expect("--application-task");
            let user = flag(&args, "--user").unwrap_or_else(|| "system".into());
            complete_workflow_task(&app_task, &user, flag(&args, "--transition").as_deref())
        }
        "complete-engine-task" => {
            let task = flag(&args, "--task").expect("--task");
            let user = flag(&args, "--user").unwrap_or_else(|| "system".into());
            complete_engine_task(&task, &user, flag(&args, "--transition").as_deref())
                .map(|_| "ok".into())
        }
        "reclaim" => {
            let batch = flag(&args, "--batch")
                .or_else(|| args.get(2).cloned())
                .and_then(|s| s.parse().ok())
                .unwrap_or(20);
            reclaim_stale_jobs(batch).map(|n| format!("reclaimed={n}"))
        }
        "reclaim-instance" => {
            let id = flag(&args, "--instance").expect("--instance");
            reclaim_stale_jobs_for_instance(&id).map(|n| format!("reclaimed={n}"))
        }
        "status" => workflow_status().map(|s| {
            format!(
                "workflow status (Rust):\n  definitions: {}\n  instances: {} ({} active, {} completed, {} failed)\n  engine tasks ready: {}\n  pending jobs: {}\n  pending receipts: {}",
                s.definition_count, s.instance_total, s.instance_active, s.instance_completed,
                s.instance_failed, s.ready_engine_tasks, s.pending_jobs, s.pending_receipts
            )
        }),
        "reconcile" => reconcile_workflows().map(|r| format!(
            "reconcile pass:\n  started instances: {}\n  materialized tasks: {}",
            r.started_instances, r.materialized_tasks
        )),
        "poll" => {
            let worker = flag(&args, "--worker")
                .or_else(|| args.get(2).filter(|s| !s.starts_with('-')).cloned())
                .unwrap_or_else(|| "workflow-cli".into());
            let positional_batch = args.get(3).filter(|s| !s.starts_with('-')).cloned();
            let batch = flag(&args, "--batch").or(positional_batch).and_then(|s| s.parse().ok()).unwrap_or(10);
            run_due_jobs(&worker, batch).map(|r| {
                format!(
                    "poll pass (worker {worker}, batch {batch}):\n  reclaimed stale leases: {}\n  claimed jobs: {}\n  fired timers: {}\n  completed jobs: {}\n  failed jobs: {}",
                    r.reclaimed, r.claimed.len(), r.fired, r.completed, r.failed
                )
            })
        }
        "reset-dev" | "reset:dev" => {
            if !args.iter().any(|a| a == "--yes") {
                Err(workflow::WorkflowError::generic("reset-dev is destructive; pass --yes"))
            } else {
                reset_dev_workflows().map(|rows| {
                    let mut out = String::from("DEV workflow reset complete:");
                    for (table, n) in rows { out.push_str(&format!("\n  {table}: {n} row(s) deleted")); }
                    out
                })
            }
        }
        _ => {
            eprintln!("usage: re-workflow start-deal --id <dealId>");
            eprintln!("       re-workflow start-contract --id <contractId>");
            eprintln!(
                "       re-workflow timer --instance <id> [--node closing_date_timer] [--date ISO]"
            );
            eprintln!("       re-workflow complete-task --application-task <id> [--user id]");
            eprintln!("       re-workflow complete-engine-task --task <id> [--user id]");
            eprintln!("       re-workflow reclaim [--batch N]");
            eprintln!("       re-workflow reclaim-instance --instance <id>");
            eprintln!("       re-workflow status");
            eprintln!("       re-workflow reconcile");
            eprintln!("       re-workflow poll [--worker id] [--batch N]");
            eprintln!("       re-workflow reset-dev --yes");
            std::process::exit(2);
        }
    };
    match out {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
