use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, TechCockpitDao};
use domain::{TechCockpitSnapshot, TechCommandRequest, TechCommandResult};
use serde_json::{json, Value};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait TechCockpitRepository: Send {
    async fn snapshot(&mut self, selected: Option<&str>) -> DbResult<TechCockpitSnapshot>;
    async fn clear_active_work(&mut self) -> DbResult<u64>;
    async fn set_active_work(&mut self, id: &str, active: bool, actor: &str) -> DbResult<()>;
    async fn story_location(&mut self, id: &str) -> DbResult<Option<(String, bool)>>;
    async fn story_status(&mut self, id: &str, status: &str) -> DbResult<()>;
    async fn active_agent_work(&mut self, id: &str) -> DbResult<Vec<Value>>;
    async fn set_dispatch_options(
        &mut self,
        id: &str,
        stop: Option<&str>,
        launch_intent: Option<&str>,
    ) -> DbResult<u64>;
    async fn withdraw_ready(&mut self, id: &str) -> DbResult<(u64, i64)>;
    async fn stage_story_for_batch(&mut self, id: &str, actor: &str) -> DbResult<(String, i64)>;
    async fn unstage_story_from_batch(&mut self, id: &str) -> DbResult<u64>;
    async fn staged_story_ids(&mut self) -> DbResult<Vec<String>>;
    async fn cancel_batch(&mut self, id: &str) -> DbResult<u64>;
    async fn schedule_flight(
        &mut self,
        when: &str,
        actor: &str,
        label: Option<&str>,
    ) -> DbResult<(String, i64)>;
    async fn launch_flight(&mut self, actor: &str) -> DbResult<Option<(String, i64, i64)>>;
}

#[async_trait]
impl TechCockpitRepository for TechCockpitDao {
    async fn snapshot(&mut self, selected: Option<&str>) -> DbResult<TechCockpitSnapshot> {
        TechCockpitDao::snapshot(self, selected).await
    }

    async fn clear_active_work(&mut self) -> DbResult<u64> {
        TechCockpitDao::clear_active_work(self).await
    }

    async fn set_active_work(&mut self, id: &str, active: bool, actor: &str) -> DbResult<()> {
        TechCockpitDao::set_active_work(self, id, active, actor).await
    }

    async fn story_location(&mut self, id: &str) -> DbResult<Option<(String, bool)>> {
        TechCockpitDao::story_location(self, id).await
    }

    async fn story_status(&mut self, id: &str, status: &str) -> DbResult<()> {
        TechCockpitDao::story_status(self, id, status).await
    }

    async fn active_agent_work(&mut self, id: &str) -> DbResult<Vec<Value>> {
        TechCockpitDao::active_agent_work(self, id).await
    }

    async fn set_dispatch_options(
        &mut self,
        id: &str,
        stop: Option<&str>,
        launch_intent: Option<&str>,
    ) -> DbResult<u64> {
        TechCockpitDao::set_dispatch_options(self, id, stop, launch_intent).await
    }

    async fn withdraw_ready(&mut self, id: &str) -> DbResult<(u64, i64)> {
        TechCockpitDao::withdraw_ready(self, id).await
    }

    async fn stage_story_for_batch(&mut self, id: &str, actor: &str) -> DbResult<(String, i64)> {
        TechCockpitDao::stage_story_for_batch(self, id, actor).await
    }

    async fn unstage_story_from_batch(&mut self, id: &str) -> DbResult<u64> {
        TechCockpitDao::unstage_story_from_batch(self, id).await
    }

    async fn staged_story_ids(&mut self) -> DbResult<Vec<String>> {
        TechCockpitDao::staged_story_ids(self).await
    }

    async fn cancel_batch(&mut self, id: &str) -> DbResult<u64> {
        TechCockpitDao::cancel_batch(self, id).await
    }

    async fn schedule_flight(
        &mut self,
        when: &str,
        actor: &str,
        label: Option<&str>,
    ) -> DbResult<(String, i64)> {
        TechCockpitDao::schedule_flight(self, when, actor, label).await
    }

    async fn launch_flight(&mut self, actor: &str) -> DbResult<Option<(String, i64, i64)>> {
        TechCockpitDao::launch_flight(self, actor).await
    }
}

pub struct TechCockpitService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: TechCockpitRepository> TechCockpitService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn snapshot(
        &mut self,
        selected: Option<&str>,
        context: &ServiceContext,
    ) -> Result<TechCockpitSnapshot, CoreServiceError> {
        const OP: &str = "tech.cockpitSnapshot";
        let decision = authorize(
            &self.runtime,
            "tech",
            "tech.access",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.snapshot(selected).await.map_err(Into::into);
        audit_result(&self.runtime, "tech", OP, context, decision, &result).await?;
        result
    }

    pub async fn command(
        &mut self,
        request: TechCommandRequest,
        context: &ServiceContext,
    ) -> Result<TechCommandResult, CoreServiceError> {
        const OP: &str = "tech.command";
        let decision = authorize(
            &self.runtime,
            "tech",
            "tech.operate",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let actor = context.actor.id.as_deref().unwrap_or("tech-operator");
        let id = request.story_id.as_deref().unwrap_or("").trim();

        let result: Result<TechCommandResult, CoreServiceError> = async {
            let ok = |message: String| {
                Ok(TechCommandResult {
                    ok: true,
                    message,
                    data: Value::Null,
                })
            };
            let ok_data = |message: String, data: Value| {
                Ok(TechCommandResult {
                    ok: true,
                    message,
                    data,
                })
            };

            match request.action.as_str() {
                "setActiveWork" => {
                    require_story_id(id)?;
                    let active = request.active.unwrap_or(true);
                    self.repository.set_active_work(id, active, actor).await?;
                    ok_data(
                        if active {
                            format!("{id} added to Active Work.")
                        } else {
                            format!("{id} removed from Active Work.")
                        },
                        json!({"storyId": id, "active": active}),
                    )
                }
                "clearWorkbench" => {
                    let cleared = self.repository.clear_active_work().await?;
                    ok_data(
                        format!(
                            "Cleared {cleared} stories from the Workbench. Story statuses were not changed."
                        ),
                        json!({"cleared": cleared}),
                    )
                }
                "goodToGo" => {
                    require_story_id(id)?;
                    let active = self.repository.active_agent_work(id).await?;
                    if let Some(work) = active.first() {
                        return Err(CoreServiceError::business(
                            "CONFLICT",
                            format!(
                                "{id} is already with Forge ({}).",
                                work["state"].as_str().unwrap_or("active")
                            ),
                        ));
                    }
                    self.repository.set_active_work(id, false, actor).await?;
                    self.repository.story_status(id, "Ready").await?;
                    if self
                        .repository
                        .set_dispatch_options(id, None, None)
                        .await?
                        == 0
                    {
                        return Err(CoreServiceError::business(
                            "CONFLICT",
                            "No queued work item to hand off — this story is already running.",
                        ));
                    }
                    ok_data(
                        format!(
                            "{id} handed to Forge for the full chain. Ready queued a real work item."
                        ),
                        json!({"storyId": id}),
                    )
                }
                "scopedRun" => {
                    require_story_id(id)?;
                    let stop = request.stop_after.as_deref().unwrap_or("");
                    if !matches!(stop, "scout" | "architect" | "lead") {
                        return Err(CoreServiceError::business(
                            "VALIDATION",
                            format!("Unsupported scoped stop: {stop}"),
                        ));
                    }
                    let launch_intent = request.launch_intent.as_deref();
                    if launch_intent.is_some_and(|value| {
                        !matches!(value, "SOLO" | "SMITH" | "SPLIT" | "HOLD")
                    }) {
                        return Err(CoreServiceError::business(
                            "VALIDATION",
                            format!(
                                "Unsupported launch intent: {}",
                                launch_intent.unwrap_or_default()
                            ),
                        ));
                    }

                    let active = self.repository.active_agent_work(id).await?;
                    if let Some(work) = active.iter().find(|work| {
                        matches!(
                            work["state"].as_str(),
                            Some("Claimed" | "Running" | "Paused")
                        )
                    }) {
                        return Err(CoreServiceError::business(
                            "CONFLICT",
                            format!(
                                "{id} is already executing ({}); a live run cannot be re-scoped.",
                                work["state"].as_str().unwrap_or("active")
                            ),
                        ));
                    }
                    if !active.iter().any(|work| work["state"] == "Ready") {
                        self.repository.story_status(id, "Ready").await?;
                    }
                    if self
                        .repository
                        .set_dispatch_options(id, Some(stop), launch_intent)
                        .await?
                        == 0
                    {
                        return Err(CoreServiceError::business(
                            "CONFLICT",
                            "No queued work item to scope — this story is already running.",
                        ));
                    }
                    ok_data(
                        format!("{id} queued through {stop}; it remains on the Workbench for review."),
                        json!({
                            "storyId": id,
                            "stopAfter": stop,
                            "launchIntent": launch_intent,
                        }),
                    )
                }
                "moveStoryBucket" => {
                    require_story_id(id)?;
                    let target = normalize_bucket(request.target.as_deref().unwrap_or(""))
                        .ok_or_else(|| {
                            CoreServiceError::business(
                                "VALIDATION",
                                format!(
                                    "Unknown column: {}",
                                    request.target.as_deref().unwrap_or("")
                                ),
                            )
                        })?;

                    let source = if let Some(source) =
                        normalize_bucket(request.source.as_deref().unwrap_or(""))
                    {
                        source
                    } else {
                        let (status, active) = self
                            .repository
                            .story_location(id)
                            .await?
                            .ok_or_else(|| {
                                CoreServiceError::business(
                                    "NOT_FOUND",
                                    format!("Story not found: {id}"),
                                )
                            })?;
                        bucket_from_status(&status, active)
                    };

                    if source == target {
                        return ok_data(
                            "already there".into(),
                            json!({"storyId": id, "source": source, "target": target}),
                        );
                    }

                    if (source == "bench" && target != "bench")
                        || target == "batch"
                        || target == "engine"
                    {
                        self.repository.set_active_work(id, false, actor).await?;
                    }

                    if source == "batch" && target != "batch" {
                        self.repository.unstage_story_from_batch(id).await?;
                    }

                    if target == "bench" {
                        self.repository.set_active_work(id, true, actor).await?;
                        let note = if source == "engine" {
                            withdrawal_note(self.repository.withdraw_ready(id).await?)
                        } else {
                            None
                        };
                        return ok_data(
                            note.clone().unwrap_or_else(|| format!("{id} moved to Work Bench.")),
                            json!({
                                "storyId": id,
                                "source": source,
                                "target": target,
                                "note": note,
                            }),
                        );
                    }

                    if target == "engine" {
                        self.repository.story_status(id, "Ready").await?;
                        let note =
                            "handed to the engine — status Ready queued a real work item".to_owned();
                        return ok_data(
                            note.clone(),
                            json!({
                                "storyId": id,
                                "source": source,
                                "target": target,
                                "note": note,
                            }),
                        );
                    }

                    let status = status_for_bucket(target).ok_or_else(|| {
                        CoreServiceError::business(
                            "VALIDATION",
                            format!("Unsupported bucket: {target}"),
                        )
                    })?;

                    let note = if source == "engine" {
                        let note = withdrawal_note(self.repository.withdraw_ready(id).await?);
                        self.repository.story_status(id, status).await?;
                        note
                    } else {
                        self.repository.story_status(id, status).await?;
                        None
                    };

                    if target == "batch" {
                        let (batch_id, members) =
                            self.repository.stage_story_for_batch(id, actor).await?;
                        let batch_note = format!("staged in the table ({members} in the batch)");
                        return ok_data(
                            batch_note.clone(),
                            json!({
                                "storyId": id,
                                "source": source,
                                "target": target,
                                "batchId": batch_id,
                                "members": members,
                                "note": batch_note,
                            }),
                        );
                    }

                    ok_data(
                        note.clone().unwrap_or_else(|| format!("{id} moved to {target}.")),
                        json!({
                            "storyId": id,
                            "source": source,
                            "target": target,
                            "note": note,
                        }),
                    )
                }
                "moveWorkbench" => {
                    require_story_id(id)?;
                    let target = normalize_bucket(request.target.as_deref().unwrap_or(""))
                        .ok_or_else(|| {
                            CoreServiceError::business(
                                "VALIDATION",
                                format!(
                                    "Unsupported Workbench destination: {}",
                                    request.target.as_deref().unwrap_or("")
                                ),
                            )
                        })?;
                    if !matches!(target, "backlog" | "closed" | "next") {
                        return Err(CoreServiceError::business(
                            "VALIDATION",
                            format!("Unsupported Workbench destination: {target}"),
                        ));
                    }
                    let status = status_for_bucket(target).expect("validated lifecycle target");
                    let note = withdrawal_note(self.repository.withdraw_ready(id).await?);
                    self.repository.set_active_work(id, false, actor).await?;
                    self.repository.story_status(id, status).await?;
                    ok_data(
                        format!(
                            "{id} moved to {status}.{}",
                            note.as_deref()
                                .map(|value| format!(" {value}"))
                                .unwrap_or_default()
                        ),
                        json!({"storyId": id, "target": target, "note": note}),
                    )
                }
                "cancelFlight" => {
                    let batch = request.batch_id.as_deref().unwrap_or("").trim();
                    if batch.is_empty() {
                        return Err(CoreServiceError::business(
                            "VALIDATION",
                            "Missing Flight id.",
                        ));
                    }
                    let cancelled = self.repository.cancel_batch(batch).await?;
                    if cancelled == 0 {
                        return Err(CoreServiceError::business(
                            "CONFLICT",
                            "That Flight is not waiting to fire.",
                        ));
                    }
                    ok_data(
                        "Scheduled Flight cancelled. Nothing was dispatched.".into(),
                        json!({"batchId": batch, "cancelled": cancelled}),
                    )
                }
                "launchFlight" => match self.repository.launch_flight(actor).await? {
                    None => ok_data(
                        "Nothing is staged in the current Flight.".into(),
                        json!({"queued": 0, "failed": []}),
                    ),
                    Some((batch_id, queued, stamped)) => ok_data(
                        format!(
                            "Flight launched: {queued} stories queued for Forge; {stamped} routing stamps applied."
                        ),
                        json!({
                            "batchId": batch_id,
                            "queued": queued,
                            "stamped": stamped,
                            "failed": [],
                        }),
                    ),
                },
                "scheduleFlight" => {
                    let when = request.scheduled_for.as_deref().unwrap_or("").trim();
                    if when.is_empty()
                        || chrono::DateTime::parse_from_rfc3339(when).is_err()
                    {
                        return Err(CoreServiceError::business(
                            "VALIDATION",
                            format!("The Flight time could not be read: {when}"),
                        ));
                    }
                    if self.repository.staged_story_ids().await?.is_empty() {
                        return Err(CoreServiceError::business(
                            "VALIDATION",
                            "Nothing is staged in the current Flight.",
                        ));
                    }
                    let (batch_id, stories) = self
                        .repository
                        .schedule_flight(when, actor, request.label.as_deref())
                        .await?;
                    ok_data(
                        format!("Scheduled {stories} stories for {when}."),
                        json!({
                            "batchId": batch_id,
                            "stories": stories,
                            "scheduledFor": when,
                        }),
                    )
                }
                other => Err(CoreServiceError::business(
                    "VALIDATION",
                    format!("Unknown TECH Cockpit command: {other}"),
                )),
            }
        }
        .await;

        audit_result(&self.runtime, "tech", OP, context, decision, &result).await?;
        result
    }
}

fn require_story_id(id: &str) -> Result<(), CoreServiceError> {
    if id.is_empty() {
        return Err(CoreServiceError::business(
            "VALIDATION",
            "Missing story id.",
        ));
    }
    Ok(())
}

fn normalize_bucket(raw: &str) -> Option<&'static str> {
    let normalized = raw
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_'], "-");
    match normalized.as_str() {
        "next-version" | "nextversion" | "next" => Some("next"),
        "work-bench" | "workbench" | "active" | "bench" => Some("bench"),
        "engine-queue" | "engine-q" | "engine-run-q" | "engine-running" | "engine" => {
            Some("engine")
        }
        "backlog-batch" | "engine-batch" | "queued" | "batch" => Some("batch"),
        "backlog" => Some("backlog"),
        "open" => Some("open"),
        "closed" => Some("closed"),
        _ => None,
    }
}

fn status_for_bucket(bucket: &str) -> Option<&'static str> {
    match bucket {
        "open" => Some("In Progress"),
        "backlog" => Some("Planned"),
        "closed" => Some("Complete"),
        "next" => Some("Deferred"),
        "batch" => Some("Batched"),
        _ => None,
    }
}

fn bucket_from_status(status: &str, active: bool) -> &'static str {
    if active {
        return "bench";
    }
    match status {
        "Batched" => "batch",
        "Ready" => "engine",
        "Planned" => "backlog",
        "Complete" => "closed",
        "Deferred" => "next",
        _ => "open",
    }
}

fn withdrawal_note((withdrawn, live): (u64, i64)) -> Option<String> {
    let mut parts = Vec::new();
    if withdrawn > 0 {
        parts.push(format!(
            "withdrew {withdrawn} queued engine request{}",
            if withdrawn == 1 { "" } else { "s" }
        ));
    }
    if live > 0 {
        parts.push("the engine is ALREADY RUNNING this — the run continues".into());
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}
