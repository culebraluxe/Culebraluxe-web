//! Moved from `engine.rs` (move only): handle_join, EventInput, command_id, resolve_input_mappings, token_outcome_for_end.

#[allow(unused_imports)]
use super::*;

impl<S: TxStore> WorkflowEngine<S> {
    pub(super) fn handle_join(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        let Some(parent_id) = token.parent_token_id.clone() else {
            if let Some(transition) = node.transitions.as_ref().and_then(|ts| ts.first()) {
                let new_token = Token {
                    id: tx.new_id("id"),
                    tenant_id: token.tenant_id.clone(),
                    process_instance_id: token.process_instance_id.clone(),
                    parent_token_id: None,
                    node_id: transition.to.clone(),
                    status: TokenStatus::Active,
                    outcome: None,
                    required: true,
                    is_able_to_reactivate_parent: true,
                    started_at: self.now(),
                    ended_at: None,
                    version: 1,
                };
                let new_token = tx.insert_token(new_token)?;
                return self
                    .arrive_at_node(tx, &new_token, instance, graph, actor, None, variables);
            }
            return Ok(());
        };

        let _ = tx.lock_token(&parent_id)?;
        self.complete_token(tx, token, actor, TokenOutcome::Completed)?;
        if tx.count_required_active_siblings(&parent_id)? > 0 {
            return Ok(());
        }

        for row in tx.list_optional_active_siblings(&parent_id)? {
            tx.complete_token(&row.id, TokenOutcome::Skipped, self.now())?;
            self.event(
                tx,
                EventInput {
                    process_instance_id: instance.id.clone(),
                    token_id: Some(row.id.clone()),
                    event_type: "token.skipped",
                    node_id: Some(row.node_id.clone()),
                    actor: actor.to_string(),
                    ..Default::default()
                },
            )?;
            for task in tx.open_tasks_for_token(&row.id)? {
                let mut next = task.clone();
                next.status = TaskStatus::Obsolete;
                next.version += 1;
                let _ = tx.cas_task(&next)?;
                self.event(
                    tx,
                    EventInput {
                        process_instance_id: instance.id.clone(),
                        task_id: Some(task.id),
                        event_type: "task.obsoleted",
                        actor: actor.to_string(),
                        data: json!({"reason": "branch skipped"}),
                        ..Default::default()
                    },
                )?;
            }
            for mut job in tx.open_jobs_for_token(&row.id)? {
                job.status = JobStatus::Cancelled;
                job.locked_by = None;
                job.locked_until = None;
                tx.update_job(&job)?;
                self.event(
                    tx,
                    EventInput {
                        process_instance_id: instance.id.clone(),
                        token_id: Some(row.id.clone()),
                        job_id: Some(job.id),
                        event_type: "job.cancelled",
                        actor: actor.to_string(),
                        data: json!({"type": job.job_type, "reason": "branch skipped"}),
                        ..Default::default()
                    },
                )?;
            }
        }

        let branch_ids: Vec<_> = tx
            .list_children(&parent_id)?
            .into_iter()
            .map(|t| t.id)
            .collect();
        let Some(transition) = node.transitions.as_ref().and_then(|ts| ts.first()).cloned() else {
            return self.check_process_completion(tx, &instance.id, actor);
        };

        let new_token = Token {
            id: tx.new_id("id"),
            tenant_id: token.tenant_id.clone(),
            process_instance_id: token.process_instance_id.clone(),
            parent_token_id: Some(parent_id),
            node_id: transition.to,
            status: TokenStatus::Active,
            outcome: None,
            required: true,
            is_able_to_reactivate_parent: true,
            started_at: self.now(),
            ended_at: None,
            version: 1,
        };
        let new_token = tx.insert_token(new_token)?;
        self.event(
            tx,
            EventInput {
                process_instance_id: instance.id.clone(),
                token_id: Some(new_token.id.clone()),
                event_type: "token.joined",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"joinNodeId": node.id, "branches": branch_ids, "resultTokenId": new_token.id}),
                ..Default::default()
            },
        )?;
        self.arrive_at_node(tx, &new_token, instance, graph, actor, None, variables)
    }

    pub(super) fn handle_timer(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        let timer = node.timer.clone().unwrap_or_default();
        let mut due_at = self.now();
        if let Some(abs) = &timer.due_at {
            if let Ok(parsed) = abs.parse::<i64>() {
                due_at = parsed;
            }
        } else if let Some(var) = &timer.due_at_variable {
            if let Some(raw) = variables.get(var) {
                if let Some(n) = raw.as_i64() {
                    due_at = n;
                } else if let Some(s) = raw.as_str() {
                    if let Ok(parsed) = s.parse::<i64>() {
                        due_at = parsed;
                    }
                }
            }
        }
        let job = Job {
            id: tx.new_id("id"),
            tenant_id: token.tenant_id.clone(),
            process_instance_id: Some(instance.id.clone()),
            token_id: Some(token.id.clone()),
            job_type: "timer".into(),
            due_at,
            status: JobStatus::Pending,
            locked_by: None,
            locked_until: None,
            attempts: 0,
            max_attempts: 5,
            payload: json!({"nodeId": node.id, "transition": timer.transition}),
            last_error: None,
            created_at: self.now(),
            updated_at: self.now(),
            completed_at: None,
        };
        let job = tx.insert_job(job)?;
        self.event(
            tx,
            EventInput {
                tenant_id: token.tenant_id.clone(),
                process_instance_id: instance.id.clone(),
                token_id: Some(token.id.clone()),
                job_id: Some(job.id),
                event_type: "timer.scheduled",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"dueAt": due_at.to_string(), "transition": timer.transition}),
                ..Default::default()
            },
        )
    }

    pub(super) fn handle_command(
        &self,
        tx: &mut dyn Store,
        token: &Token,
        node: &NodeDefinition,
        instance: &ProcessInstance,
        graph: &ProcessGraph,
        actor: &str,
        variables: &Value,
    ) -> Result<()> {
        let app = self.app.as_ref().ok_or_else(|| {
            WorkflowError::missing_port(format!(
                "Application port not configured for command node '{}'",
                node.id
            ))
        })?;
        let command_type = node.command_type.clone().ok_or_else(|| {
            WorkflowError::generic(format!("Command node '{}' has no commandType", node.id))
        })?;
        let visit_sequence = tx.command_visit_count(&instance.id, &node.id)? + 1;
        let command_id = command_id(&instance.id, &node.id, visit_sequence);
        let input = resolve_input_mappings(node.input_mappings.as_ref(), variables);
        let request = ApplicationCommandRequest {
            command_id: command_id.clone(),
            command_type: command_type.clone(),
            subject_type: instance.subject_type.clone(),
            subject_id: instance.subject_id.clone(),
            correlation_id: instance.id.clone(),
            causation_id: None,
            input: input.clone(),
        };
        self.event(
            tx,
            EventInput {
                process_instance_id: instance.id.clone(),
                token_id: Some(token.id.clone()),
                event_type: "command.requested",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"commandId": command_id, "commandType": command_type}),
                ..Default::default()
            },
        )?;
        let result = app.execute_command(&request);
        tx.insert_command(ProcessCommand {
            process_instance_id: instance.id.clone(),
            token_id: token.id.clone(),
            node_id: node.id.clone(),
            visit_sequence,
            command_id: command_id.clone(),
            command_type: command_type.clone(),
            subject_type: request.subject_type,
            subject_id: request.subject_id,
            correlation_id: request.correlation_id,
            causation_id: request.causation_id,
            input,
            outcome: result.outcome.as_str().to_string(),
            message: result.message.clone(),
        })?;

        if result.outcome == ApplicationCommandOutcome::Success {
            self.event(
                tx,
                EventInput {
                    process_instance_id: instance.id.clone(),
                    token_id: Some(token.id.clone()),
                    event_type: "command.completed",
                    node_id: Some(node.id.clone()),
                    actor: actor.to_string(),
                    data: json!({"commandId": command_id, "commandType": command_type}),
                    ..Default::default()
                },
            )?;
            let current = if self.app.is_some() {
                self.refresh_facts(tx, instance, variables)?
            } else {
                variables.clone()
            };
            let transition_name = node.transition.clone().or_else(|| {
                node.transitions
                    .as_ref()
                    .and_then(|ts| ts.first().map(|t| t.name.clone()))
            });
            let transition = node
                .transitions
                .as_ref()
                .and_then(|ts| {
                    ts.iter()
                        .find(|t| Some(&t.name) == transition_name.as_ref())
                        .or_else(|| ts.first())
                })
                .cloned()
                .ok_or_else(|| {
                    WorkflowError::generic(format!(
                        "Command node {} has no success transition",
                        node.id
                    ))
                })?;
            self.move_token(tx, token, &transition.to, &transition.name, actor)?;
            let fresh = tx.get_token(&token.id)?;
            let mut inst = instance.clone();
            inst.variables = current.clone();
            return self.arrive_at_node(tx, &fresh, &inst, graph, actor, None, &current);
        }

        self.event(
            tx,
            EventInput {
                process_instance_id: instance.id.clone(),
                token_id: Some(token.id.clone()),
                event_type: "command.failed",
                node_id: Some(node.id.clone()),
                actor: actor.to_string(),
                data: json!({"commandId": command_id, "outcome": result.outcome.as_str(), "message": result.message}),
                ..Default::default()
            },
        )?;
        let outcome = if result.outcome == ApplicationCommandOutcome::Conflict {
            ProcessOutcome::Conflict
        } else {
            ProcessOutcome::Failed
        };
        self.terminate_process(tx, &instance.id, actor, outcome, result.message.as_deref())
    }

    pub(super) fn refresh_facts(
        &self,
        tx: &mut dyn Store,
        instance: &ProcessInstance,
        variables: &Value,
    ) -> Result<Value> {
        let Some(app) = &self.app else {
            return Ok(variables.clone());
        };
        let (Some(st), Some(sid)) = (&instance.subject_type, &instance.subject_id) else {
            return Ok(variables.clone());
        };
        let facts = app.read_facts(&WorkflowSubject {
            subject_type: st.clone(),
            subject_id: sid.clone(),
        });
        let mut merged = variables.clone();
        merge_json(&mut merged, &facts);
        tx.update_instance_variables(&instance.id, merged.clone())?;
        Ok(merged)
    }

    pub(super) fn event(&self, tx: &mut dyn Store, input: EventInput) -> Result<()> {
        tx.insert_event(ProcessEvent {
            id: 0,
            tenant_id: input.tenant_id,
            process_instance_id: input.process_instance_id,
            token_id: input.token_id,
            task_id: input.task_id,
            job_id: input.job_id,
            event_type: input.event_type.to_string(),
            node_id: input.node_id,
            actor: input.actor,
            data: input.data,
            created_at: self.now(),
        })
    }
}

#[derive(Default)]
pub(super) struct EventInput {
    pub(super) tenant_id: Option<String>,
    pub(super) process_instance_id: String,
    pub(super) token_id: Option<String>,
    pub(super) task_id: Option<String>,
    pub(super) job_id: Option<String>,
    pub(super) event_type: &'static str,
    pub(super) node_id: Option<String>,
    pub(super) actor: String,
    pub(super) data: Value,
}

pub fn command_id(process_instance_id: &str, node_id: &str, visit_sequence: i32) -> String {
    crate::sha256::sha256_hex(
        format!("{process_instance_id}:{node_id}:{visit_sequence}").as_bytes(),
    )
}

/// Resolve command `inputMappings`.
///
/// A string value is treated as a variable name and replaced with that
/// variable when present; anything else is copied through. An empty or
/// missing mapping passes the instance variables unchanged.
pub(super) fn resolve_input_mappings(mappings: Option<&Value>, variables: &Value) -> Value {
    let Some(Value::Object(map)) = mappings else {
        return variables.clone();
    };
    if map.is_empty() {
        return variables.clone();
    }
    let mut input = Value::object();
    for (k, v) in map {
        if let Some(path) = v.as_str() {
            if let Some(resolved) = variables.get(path) {
                input.insert(k.clone(), resolved.clone());
                continue;
            }
        }
        input.insert(k.clone(), v.clone());
    }
    input
}

pub(super) fn token_outcome_for_end(end: ProcessOutcome) -> TokenOutcome {
    match end {
        ProcessOutcome::Completed => TokenOutcome::Completed,
        ProcessOutcome::Cancelled => TokenOutcome::Cancelled,
        ProcessOutcome::Failed | ProcessOutcome::Conflict => TokenOutcome::Failed,
    }
}
