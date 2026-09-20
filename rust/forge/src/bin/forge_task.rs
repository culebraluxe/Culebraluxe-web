//! Mutation door used by workflow_app/forge/forge-engine-runtime.ts.
//! Replaces `new WorkflowEngine` for start/complete/claim/release/cancel.

use forge::engine::db_writer::DbForgeStateWriter;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::port::ForgeApplicationPort;
use forge::engine::xml::definition_from_xml;
use forge::engine::xml::FORGE_SDLC_V6_XML;
use std::env;
use std::sync::Arc;
use workflow::{
    CancelProcessParams, CompleteTaskParams, EngineOptions, NeonStore, StartProcessParams, Value,
    WorkflowEngine, WorkflowSubject,
};

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn engine() -> WorkflowEngine<NeonStore> {
    let store = NeonStore::connect_from_env().expect("neon");
    let writer = DbForgeStateWriter::connect_env().ok();
    let port = ForgeApplicationPort::new(
        match writer {
            Some(w) => Arc::new(w),
            None => Arc::new(forge::engine::writer::NullWriter),
        },
        None,
        None,
    );
    let eng = WorkflowEngine::new(
        store,
        EngineOptions {
            app: Some(Box::new(port)),
            now: Box::new(now_millis),
        },
    );
    if let Ok(def) = definition_from_xml(FORGE_SDLC_V6_XML) {
        let _ = eng.seed_definition(def);
    } else {
        let _ = eng.seed_definition(forge_sdlc_definition());
    }
    eng
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let r = match cmd {
        "start" => {
            let story = flag(&args, "--story").expect("--story");
            let work_type = flag(&args, "--work-type").unwrap_or_else(|| "FEATURE".into());
            let mut vars = Value::object();
            vars.insert("workType", Value::String(work_type));
            engine()
                .start_process(StartProcessParams {
                    definition_key: "FORGE_SDLC".into(),
                    version: Some(6),
                    business_key: Some(story.clone()),
                    variables: vars,
                    started_by: "forge".into(),
                    tenant_id: None,
                    subject: Some(WorkflowSubject {
                        subject_type: "story".into(),
                        subject_id: story,
                    }),
                })
                .map(|r| r.process_instance_id)
        }
        "complete" => {
            let task = flag(&args, "--task").expect("--task");
            let user = flag(&args, "--user").unwrap_or_else(|| "forge".into());
            let transition = flag(&args, "--transition").unwrap_or_else(|| "complete".into());
            engine()
                .complete_task(CompleteTaskParams {
                    task_id: task,
                    user_id: user,
                    form_data: Value::object(),
                    transition_name: Some(transition),
                })
                .map(|_| "ok".into())
        }
        "claim" => {
            let task = flag(&args, "--task").expect("--task");
            let user = flag(&args, "--user").unwrap_or_else(|| "forge".into());
            engine().claim_task(&task, &user).map(|_| "ok".into())
        }
        "release" => {
            let task = flag(&args, "--task").expect("--task");
            let user = flag(&args, "--user").unwrap_or_else(|| "forge".into());
            engine().release_task(&task, &user).map(|_| "ok".into())
        }
        "cancel" => {
            let id = flag(&args, "--instance").expect("--instance");
            engine()
                .cancel_process(CancelProcessParams {
                    process_instance_id: id,
                    actor: "forge".into(),
                    reason: flag(&args, "--reason"),
                })
                .map(|_| "ok".into())
        }
        _ => {
            eprintln!("usage: forge-task start|complete|claim|release|cancel ...");
            std::process::exit(2);
        }
    };
    match r {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
