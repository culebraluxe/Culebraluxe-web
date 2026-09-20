//! Residential transaction host. Replaces workflow_app/runtime.ts engine door.

use forge::engine::re_runtime::{
    complete_workflow_task, reconcile_closing_timer, reconcile_deadline_timer,
    start_residential_transaction,
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
        _ => {
            eprintln!("usage: re-workflow start-deal --id <dealId>");
            eprintln!("       re-workflow start-contract --id <contractId>");
            eprintln!("       re-workflow timer --instance <id> [--node closing_date_timer] [--date ISO]");
            eprintln!("       re-workflow complete-task --application-task <id> [--user id]");
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
