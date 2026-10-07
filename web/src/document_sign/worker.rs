//! The two background jobs native signing cannot work without.
//!
//! * **Finalize.** When the last signer completes, the command emits `DOCUMENT_SIGN_READY_TO_FINALIZE`. Nothing used to
//!   listen, so a fully signed envelope stayed `signed` forever: no sealed PDF, no certificate, no completion email.
//!   [`DocumentSignFinalizeSubscriber`] listens, and runs `documentSign.finalize` through the same durable dispatcher a
//!   person would.
//! * **Sweep.** Overdue signers and envelopes expire, and signers who have gone quiet get their reminders
//!   (`documentSign.sweepDue`). Nothing ran it either; [`spawn_sweeper`] does, on a timer.
//!
//! Both run as narrow SYSTEM actors that Casbin admits for exactly these operations (`security/entitlements.rs`), so a
//! worker holds no standing authority beyond its one job.

use crate::mq_runtime::{MqSubscriber, MqSubscriberError};
use crate::{CommandDispatcher, ServiceRegistry};
use async_trait::async_trait;
use db::OutboxDelivery;
use serde_json::Map;
use services::{CommandOutcome, CommandRequest, ServiceActor, ServiceActorKind, ServiceContext};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub const FINALIZE_SUBSCRIPTION_ID: &str = "document-sign.finalize";
pub const READY_TO_FINALIZE_ROUTING_KEY: &str = "DOCUMENT_SIGN_READY_TO_FINALIZE";
pub const DOCUMENT_SIGN_FINALIZER_ACTOR: &str = "document-sign-finalizer";
pub const DOCUMENT_SIGN_SWEEPER_ACTOR: &str = "document-sign-sweeper";

/// How often the sweeper looks. Reminders are counted in days, so a quarter of an hour is far finer than needed and
/// still cheap: one indexed query when nothing is due.
pub const SWEEP_PERIOD: Duration = Duration::from_secs(15 * 60);

fn system_context(
    actor: &str,
    correlation_id: String,
    causation_id: Option<String>,
) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(actor.into()),
            kind: ServiceActorKind::System,
        },
        correlation_id,
        causation_id,
        principal: None,
    }
}

/// Run one command through the dispatcher under its service's scheduling policy, as the real callers do.
async fn run(
    commands: &CommandDispatcher,
    registry: &ServiceRegistry,
    request: CommandRequest,
    context: ServiceContext,
) -> Result<(), String> {
    let (domain, operation, payload) = commands
        .scheduling_route(&request)
        .ok_or_else(|| format!("{} has no scheduler route.", request.command_type))?;
    let dispatcher = commands.clone();
    let scheduled = request.clone();
    let result = registry
        .run_task(domain, operation, &payload, async move {
            dispatcher.execute(&scheduled, &context).await
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    if result.outcome == CommandOutcome::Success {
        return Ok(());
    }
    let code = result
        .error
        .as_ref()
        .map(|error| error.code.as_str())
        .unwrap_or(result.outcome.as_str());
    let message = result
        .error
        .as_ref()
        .map(|error| error.message.as_str())
        .or(result.message.as_deref())
        .unwrap_or("the command failed");
    Err(format!("{code}: {message}"))
}

pub struct DocumentSignFinalizeSubscriber {
    commands: CommandDispatcher,
    registry: Arc<ServiceRegistry>,
}

impl DocumentSignFinalizeSubscriber {
    pub fn new(commands: CommandDispatcher, registry: Arc<ServiceRegistry>) -> Self {
        Self { commands, registry }
    }
}

#[async_trait]
impl MqSubscriber for DocumentSignFinalizeSubscriber {
    fn id(&self) -> &str {
        FINALIZE_SUBSCRIPTION_ID
    }

    fn routing_key(&self) -> &str {
        READY_TO_FINALIZE_ROUTING_KEY
    }

    fn max_attempts(&self) -> i32 {
        5
    }

    fn retry_backoff_seconds(&self) -> i32 {
        30
    }

    async fn handle(&self, delivery: &OutboxDelivery) -> Result<(), MqSubscriberError> {
        let signature_request_id = delivery
            .payload
            .get("signatureRequestId")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                MqSubscriberError::new(
                    "DOCUMENT_SIGN_READY_TO_FINALIZE requires signatureRequestId.",
                )
            })?;
        // One id per envelope: a redelivered event replays the first result instead of sealing twice.
        let request = CommandRequest {
            command_id: format!("document-sign:finalize:{signature_request_id}"),
            command_type: "documentSign.finalize".into(),
            aggregate_type: "signature_request".into(),
            aggregate_id: Some(signature_request_id.to_owned()),
            requested_at: delivery.occurred_at.to_rfc3339(),
            input: Map::new(),
        };
        let context = system_context(
            DOCUMENT_SIGN_FINALIZER_ACTOR,
            delivery
                .correlation_id
                .clone()
                .unwrap_or_else(|| delivery.event_id.clone()),
            Some(delivery.event_id.clone()),
        );
        run(&self.commands, &self.registry, request, context)
            .await
            .map_err(MqSubscriberError::new)
    }
}

/// Run `documentSign.sweepDue` every [`SWEEP_PERIOD`] until `shutdown`. A failed sweep is logged and tried again next
/// period; it never stops the loop.
pub fn spawn_sweeper(
    commands: CommandDispatcher,
    registry: Arc<ServiceRegistry>,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = tokio::time::sleep(SWEEP_PERIOD) => {}
            }
            let now = chrono::Utc::now();
            let request = CommandRequest {
                command_id: format!("document-sign:sweep:{}", now.timestamp_millis()),
                command_type: "documentSign.sweepDue".into(),
                aggregate_type: "signature_request".into(),
                aggregate_id: None,
                requested_at: now.to_rfc3339(),
                input: Map::new(),
            };
            let context = system_context(
                DOCUMENT_SIGN_SWEEPER_ACTOR,
                format!("document-sign-sweep-{}", now.timestamp()),
                None,
            );
            if let Err(error) = run(&commands, &registry, request, context).await {
                tracing::warn!(target: "culebraluxe::document_sign", %error, "signing sweep failed; will retry");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_finalizer_listens_for_the_event_the_signer_command_emits() {
        // The key is the event type `SignerCommandKind::Complete` pushes. A rename on one side only is a silent
        // production failure (no sealed document), so it is pinned here and read back from the producer's source.
        let producer = include_str!("../command_runtime.rs");
        assert!(
            producer.contains(&format!("\"{READY_TO_FINALIZE_ROUTING_KEY}\"")),
            "command_runtime must emit {READY_TO_FINALIZE_ROUTING_KEY}"
        );
    }

    #[test]
    fn the_sweep_runs_often_enough_for_day_granular_reminders() {
        assert!(SWEEP_PERIOD <= Duration::from_secs(3600));
    }
}
