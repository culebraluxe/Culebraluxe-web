//! Compatibility shim for the app RE port; the Forge-only composite port stays here until step 7/8.

pub use re_host::port::ReApplicationPort;

use workflow::{
    ApplicationCommandRequest, ApplicationCommandResult, ApplicationPort, Value, WorkflowSubject,
};

/// Forge commands stay on ForgeApplicationPort; RE commands on ReApplicationPort.
pub struct CompositeApplicationPort {
    pub forge: Box<dyn ApplicationPort>,
    pub re: ReApplicationPort,
}

impl ApplicationPort for CompositeApplicationPort {
    fn execute_command(&self, request: &ApplicationCommandRequest) -> ApplicationCommandResult {
        if request.command_type.starts_with("forge.") {
            self.forge.execute_command(request)
        } else {
            self.re.execute_command(request)
        }
    }

    fn read_facts(&self, subject: &WorkflowSubject) -> Value {
        match subject.subject_type.as_str() {
            "story" => self.forge.read_facts(subject),
            "deal" | "contract" => self.re.read_facts(subject),
            _ => Value::object(),
        }
    }
}
