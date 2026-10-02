//! Registry for Forge role services.
//!
//! The registry is deliberately boring: it binds a stable service key to one
//! `AbstractForgeService`. It does not schedule work, apply workflow policy,
//! or contain role-specific branching.

use std::collections::BTreeMap;

use crate::roles::service::AbstractForgeService;
use workflow::{Result, WorkflowError};

pub struct ForgeServiceRegistry<'a> {
    services: BTreeMap<&'static str, &'a dyn AbstractForgeService>,
}

impl<'a> ForgeServiceRegistry<'a> {
    pub fn new() -> Self {
        Self {
            services: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, service: &'a dyn AbstractForgeService) -> Result<()> {
        let descriptor = service.descriptor();

        if self.services.contains_key(descriptor.service_id) {
            return Err(WorkflowError::generic(format!(
                "duplicate Forge service key {:?}",
                descriptor.service_id
            )));
        }

        if let Some(existing) = self
            .services
            .values()
            .copied()
            .find(|registered| registered.descriptor().lane == descriptor.lane)
        {
            return Err(WorkflowError::generic(format!(
                "Forge lane {:?} already belongs to {}; refusing second owner {}",
                descriptor.lane,
                existing.descriptor().service_id,
                descriptor.service_id
            )));
        }

        self.services.insert(descriptor.service_id, service);
        Ok(())
    }

    pub fn resolve(&self, service_key: &str) -> Result<&'a dyn AbstractForgeService> {
        self.services.get(service_key).copied().ok_or_else(|| {
            WorkflowError::generic(format!("unknown Forge service key {service_key:?}"))
        })
    }

    pub fn resolve_node(&self, node_id: &str) -> Result<&'a dyn AbstractForgeService> {
        let mut matches = self
            .services
            .values()
            .copied()
            .filter(|service| service.supports_node(node_id));

        let Some(service) = matches.next() else {
            return Err(WorkflowError::generic(format!(
                "no registered Forge service owns node {node_id:?}"
            )));
        };

        if let Some(other) = matches.next() {
            return Err(WorkflowError::generic(format!(
                "Forge node {node_id:?} has multiple service owners: {} and {}",
                service.descriptor().service_id,
                other.descriptor().service_id
            )));
        }

        Ok(service)
    }
}

impl Default for ForgeServiceRegistry<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
    use crate::engine::facts::ForgeGateEvidence;
    use crate::engine::runtime::ActiveForgeRoleTask;
    use crate::roles::smith::SmithService;

    struct NoopRunner;

    impl ForgeRoleRunner for NoopRunner {
        fn run(
            &self,
            _node_id: &str,
            _task: &ActiveForgeRoleTask,
        ) -> Result<ForgeRoleOutcome> {
            Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence: ForgeGateEvidence::default(),
            })
        }
    }

    #[test]
    fn registry_rejects_two_services_for_the_same_lane() {
        let runner = NoopRunner;
        let first = SmithService::new(&runner);
        let second = SmithService::new(&runner);
        let mut registry = ForgeServiceRegistry::new();

        registry.register(&first).expect("first Smith owner");
        let error = registry
            .register(&second)
            .expect_err("a Forge lane may have only one service owner");

        assert!(
            error.to_string().contains("forge.smith"),
            "duplicate registration must identify the conflicting service key"
        );
    }
}
