//! Rust host for the workflow kernel. Replaces workflow_engine/lib/workflow/db.ts.

use std::env;
use workflow::{EngineOptions, NeonStore, StartProcessParams, Value, WorkflowEngine};

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    match cmd {
        "start" => {
            let def = flag(&args, "--definition").expect("--definition required");
            let subject = flag(&args, "--subject");
            let engine = host_engine();
            match engine.start_process(StartProcessParams {
                definition_key: def,
                version: None,
                business_key: subject.clone(),
                variables: Value::default(),
                started_by: flag(&args, "--actor").unwrap_or_else(|| "workflow".into()),
                tenant_id: None,
                subject: None,
            }) {
                Ok(r) => println!("{}", r.process_instance_id),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            }
        }
        "help" | _ => {
            eprintln!("usage: workflow start --definition <id> [--subject <id>] [--actor <id>]");
            if cmd != "help" {
                std::process::exit(2);
            }
        }
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn host_engine() -> WorkflowEngine<NeonStore> {
    match NeonStore::connect_from_env() {
        Ok(store) => WorkflowEngine::new(store, EngineOptions::default()),
        Err(e) => {
            eprintln!("neon: {e}");
            std::process::exit(2);
        }
    }
}
