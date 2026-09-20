use std::collections::BTreeMap;
use std::sync::Arc;
use workflow::{ApplicationCommandOutcome, ApplicationCommandRequest, ApplicationCommandResult};
use crate::engine::commands;
use crate::engine::writer::{cmd_result, optional_string, required_string, ForgeReleaseExecutor, ForgeStateWriter};

pub type Handler = Box<dyn Fn(&ApplicationCommandRequest) -> ApplicationCommandResult + Send + Sync>;

pub struct ForgeCommandRegistry { handlers: BTreeMap<String, Handler> }

impl ForgeCommandRegistry {
    pub fn build(writer: Arc<dyn ForgeStateWriter>, release: Option<Arc<dyn ForgeReleaseExecutor>>) -> Self {
        let mut handlers: BTreeMap<String, Handler> = BTreeMap::new();
        {
            let w = writer.clone();
            handlers.insert(commands::STORY_MARK_HOLD.into(), Box::new(move |req| match required_string(&req.input, "storyId") {
                Err(m) => cmd_result(&req.command_id, ApplicationCommandOutcome::ValidationFailure, Some(m)),
                Ok(id) => {
                    let reason = optional_string(&req.input, "reason").unwrap_or_else(|| "Forge engine hold command".into());
                    match w.mark_story_human_hold(&id, &reason) {
                        Ok(()) => cmd_result(&req.command_id, ApplicationCommandOutcome::Success, None),
                        Err(m) => cmd_result(&req.command_id, ApplicationCommandOutcome::PreconditionFailure, Some(m)),
                    }
                }
            }));
        }
        {
            let w = writer.clone();
            handlers.insert(commands::STORY_MARK_COMPLETE.into(), Box::new(move |req| match required_string(&req.input, "storyId") {
                Err(m) => cmd_result(&req.command_id, ApplicationCommandOutcome::ValidationFailure, Some(m)),
                Ok(id) => match w.mark_story_complete(&id) {
                    Ok(()) => cmd_result(&req.command_id, ApplicationCommandOutcome::Success, None),
                    Err(m) => cmd_result(&req.command_id, ApplicationCommandOutcome::PreconditionFailure, Some(m)),
                },
            }));
        }
        {
            let w = writer.clone();
            handlers.insert(commands::STORY_MARK_IN_PROGRESS.into(), Box::new(move |req| match required_string(&req.input, "storyId") {
                Err(m) => cmd_result(&req.command_id, ApplicationCommandOutcome::ValidationFailure, Some(m)),
                Ok(id) => match w.mark_story_in_progress(&id) {
                    Ok(()) => cmd_result(&req.command_id, ApplicationCommandOutcome::Success, None),
                    Err(m) => cmd_result(&req.command_id, ApplicationCommandOutcome::PreconditionFailure, Some(m)),
                },
            }));
        }
        {
            let w = writer.clone();
            handlers.insert(commands::RUN_APPEND_DETAIL.into(), Box::new(move |req| {
                let run_id = required_string(&req.input, "runId");
                let detail = required_string(&req.input, "detail");
                match (run_id, detail) {
                    (Ok(r), Ok(d)) => match w.append_run_detail(&r, &d) {
                        Ok(()) => cmd_result(&req.command_id, ApplicationCommandOutcome::Success, None),
                        Err(m) => cmd_result(&req.command_id, ApplicationCommandOutcome::PreconditionFailure, Some(m)),
                    },
                    (Err(m), _) | (_, Err(m)) => cmd_result(&req.command_id, ApplicationCommandOutcome::ValidationFailure, Some(m)),
                }
            }));
        }
        for ty in commands::ROUTED {
            if handlers.contains_key(*ty) { continue; }
            if commands::is_release(ty) {
                let exec = release.clone();
                let name = (*ty).to_string();
                handlers.insert(name.clone(), Box::new(move |req| match &exec {
                    Some(e) => e.execute(&name, &req.input),
                    None => cmd_result(&req.command_id, ApplicationCommandOutcome::PreconditionFailure, Some(format!("{name} requires a configured real Forge release executor"))),
                }));
            } else {
                let name = (*ty).to_string();
                handlers.insert(name.clone(), Box::new(move |req| {
                    cmd_result(&req.command_id, ApplicationCommandOutcome::PreconditionFailure, Some(format!("{name} is not executable as a command; use a claimed Forge engine task")))
                }));
            }
        }
        Self { handlers }
    }
    pub fn dispatch(&self, req: &ApplicationCommandRequest) -> ApplicationCommandResult {
        if !commands::is_routed(&req.command_type) {
            return cmd_result(&req.command_id, ApplicationCommandOutcome::NotFound, Some(format!("Not a Forge command: {} (Forge executes forge.* commands only)", req.command_type)));
        }
        match self.handlers.get(&req.command_type) {
            Some(h) => h(req),
            None => cmd_result(&req.command_id, ApplicationCommandOutcome::NotFound, Some(format!("Unknown Forge command: {}", req.command_type))),
        }
    }
}
