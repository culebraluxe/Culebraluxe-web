use crate::{Database, DbFailure, DbResult};
use serde_json::Value;
use sqlx::FromRow;

pub const AGENT_CLAIM_LOCK: i64 = 9_000_212;

#[derive(Debug, Clone, FromRow)]
pub struct ForgeAgentWorkRow {
    pub id: String,
    pub story_id: String,
    pub state: String,
    pub claimed_by: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeDecisionRow {
    pub key: String,
    pub statement: String,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProcessDefinitionRow {
    pub id: String,
    pub definition: Value,
}

#[derive(Debug, Clone, FromRow)]
pub struct StoryPacketRow {
    pub id: String,
    pub title: String,
    pub goal: Option<String>,
    pub architect_brief: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub assay_commands: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ForgeHoldRow {
    pub reason: String,
    pub originating_node: Option<String>,
}

#[derive(Clone)]
pub struct ForgeEngineDao {
    db: Database,
}

impl ForgeEngineDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn claim_specific_agent_work(
        &self,
        work_item_id: &str,
        worker_id: &str,
    ) -> DbResult<Option<ForgeAgentWorkRow>> {
        let mut tx = self
            .db
            .begin("forge_engine.claim_specific_agent_work")
            .await?;
        sqlx::query("select pg_advisory_xact_lock($1)")
            .bind(AGENT_CLAIM_LOCK)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.lock", &error))?;

        let group = sqlx::query_scalar::<_, Option<String>>(
            "select parallel_group_id::text from agent_work_item where id=$1::uuid",
        )
        .bind(work_item_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.group", &error))?
        .flatten();

        let active = if group.is_none() {
            sqlx::query_scalar::<_, String>(
                "select id::text from agent_work_item
                 where state in ('Claimed','Running','Paused')
                   and story_id=(select story_id from agent_work_item where id=$1::uuid)
                 limit 1",
            )
            .bind(work_item_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.active", &error))?
        } else {
            sqlx::query_scalar::<_, String>(
                "select id::text from agent_work_item
                 where state in ('Claimed','Running','Paused')
                   and parallel_group_id is null
                   and story_id=(select story_id from agent_work_item where id=$1::uuid)
                 limit 1",
            )
            .bind(work_item_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| {
                DbFailure::from_sqlx("forge_engine.claim_specific.active_group", &error)
            })?
        };

        if active.is_some() {
            tx.commit().await?;
            return Ok(None);
        }

        let row = sqlx::query_as::<_, ForgeAgentWorkRow>(
            "update agent_work_item
             set state='Claimed', claimed_at=now(), claimed_by=$2,
                 attempts=attempts+1, updated_at=now()
             where id=$1::uuid and state='Ready'
             returning id::text as id, story_id, state, claimed_by, role",
        )
        .bind(work_item_id)
        .bind(worker_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_specific.update", &error))?;
        tx.commit().await?;
        Ok(row)
    }

    pub async fn claim_next_agent_work(
        &self,
        worker_id: &str,
    ) -> DbResult<Option<ForgeAgentWorkRow>> {
        let mut tx = self.db.begin("forge_engine.claim_next_agent_work").await?;
        sqlx::query("select pg_advisory_xact_lock($1)")
            .bind(AGENT_CLAIM_LOCK)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_next.lock", &error))?;

        let active = sqlx::query_scalar::<_, String>(
            "select id::text from agent_work_item where state in ('Claimed','Running') limit 1",
        )
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_next.active", &error))?;
        if active.is_some() {
            tx.commit().await?;
            return Ok(None);
        }

        let row = sqlx::query_as::<_, ForgeAgentWorkRow>(
            "update agent_work_item
             set state='Claimed', claimed_at=now(), claimed_by=$1,
                 attempts=attempts+1, updated_at=now()
             where id=(
               select id from agent_work_item where state='Ready'
               order by priority desc, queued_at asc, id limit 1
             )
             returning id::text as id, story_id, state, claimed_by, role",
        )
        .bind(worker_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_next.update", &error))?;
        tx.commit().await?;
        Ok(row)
    }

    pub async fn begin_agent_work_run(&self, work_item_id: &str) -> DbResult<()> {
        sqlx::query(
            "update agent_work_item
             set state='Running', started_at=coalesce(started_at,now()), updated_at=now()
             where id=$1::uuid and state='Claimed'",
        )
        .bind(work_item_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.begin_agent_work_run", &error))?;
        Ok(())
    }

    pub async fn reject_agent_work_configuration(
        &self,
        work_item_id: &str,
        evidence: &str,
    ) -> DbResult<()> {
        sqlx::query(
            "update agent_work_item
             set state='Failed', error_text=$2, finished_at=now(), updated_at=now()
             where id=$1::uuid and state in ('Claimed','Ready')",
        )
        .bind(work_item_id)
        .bind(evidence)
        .execute(self.db.pool())
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("forge_engine.reject_agent_work_configuration", &error)
        })?;
        Ok(())
    }

    pub async fn mark_story_in_progress(&self, story_id: &str) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story
             set status='In Progress', completion=0, completed_at=null, updated_at=now()
             where id=$1",
        )
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_story_in_progress", &error))?;
        Ok(())
    }

    pub async fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story
             set status='Hold', completed_at=null,
                 notes=case
                   when nullif(trim($2),'') is null then notes
                   when notes is null or notes='' then $2
                   else notes || E'\\n' || $2
                 end,
                 updated_at=now()
             where id=$1",
        )
        .bind(story_id)
        .bind(reason.trim())
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_story_human_hold", &error))?;
        Ok(())
    }

    pub async fn mark_story_complete(&self, story_id: &str) -> DbResult<()> {
        sqlx::query(
            "update storyboard_story
             set status='Complete', completion=100,
                 completed_at=coalesce(completed_at,now()), updated_at=now()
             where id=$1",
        )
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.mark_story_complete", &error))?;
        Ok(())
    }

    pub async fn append_run_detail(&self, run_id: &str, detail: &str) -> DbResult<()> {
        let detail = detail.trim();
        if detail.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "update storyboard_story_run
             set evidence_detail=case
               when evidence_detail is null or evidence_detail=''
                 then to_char(now(),'YYYY-MM-DD HH24:MI:SS') || ' — ' || $2
               else evidence_detail || E'\\n' || to_char(now(),'YYYY-MM-DD HH24:MI:SS') || ' — ' || $2
             end,
             updated_at=now()
             where id=$1::uuid",
        )
        .bind(run_id)
        .bind(detail)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.append_run_detail", &error))?;
        Ok(())
    }

    pub async fn active_decisions(
        &self,
        domain: &str,
        limit: i64,
    ) -> DbResult<Vec<ForgeDecisionRow>> {
        sqlx::query_as::<_, ForgeDecisionRow>(
            "select key, statement, owner
             from forge_decision
             where status='active' and domain=$1
             order by promoted_at desc nulls last, key
             limit $2",
        )
        .bind(domain)
        .bind(limit.clamp(1, 20))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.active_decisions", &error))
    }

    pub async fn process_definition(
        &self,
        key: &str,
        version: i32,
    ) -> DbResult<Option<ProcessDefinitionRow>> {
        sqlx::query_as::<_, ProcessDefinitionRow>(
            "select id::text as id, definition
             from process_definitions
             where tenant_id is null and key=$1 and version=$2
             limit 1",
        )
        .bind(key)
        .bind(version)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.process_definition", &error))
    }

    pub async fn process_definition_use_count(&self, id: &str) -> DbResult<i64> {
        sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from process_instances where definition_id=$1::uuid",
        )
        .bind(id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.process_definition_use_count", &error))
    }

    pub async fn update_process_definition(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        definition: &Value,
    ) -> DbResult<()> {
        sqlx::query(
            "update process_definitions
             set name=$2, description=$3, definition=$4, status='active', updated_at=now()
             where id=$1::uuid",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(definition)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.update_process_definition", &error))?;
        Ok(())
    }

    pub async fn insert_process_definition(
        &self,
        key: &str,
        version: i32,
        name: &str,
        description: Option<&str>,
        definition: &Value,
        created_by: Option<&str>,
    ) -> DbResult<String> {
        sqlx::query_scalar::<_, String>(
            "insert into process_definitions
             (tenant_id,key,version,name,description,definition,status,created_by)
             values(null,$1,$2,$3,$4,$5,'active',$6)
             returning id::text",
        )
        .bind(key)
        .bind(version)
        .bind(name)
        .bind(description)
        .bind(definition)
        .bind(created_by)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.insert_process_definition", &error))
    }

    pub async fn open_hold(
        &self,
        process_instance_id: &str,
        task_id: Option<&str>,
        story_id: &str,
        reason: &str,
        originating_node: Option<&str>,
        failure_class: Option<&str>,
        resume_target: Option<&str>,
    ) -> DbResult<String> {
        sqlx::query_scalar::<_, String>(
            "insert into forge_hold_record(
               process_instance_id,task_id,story_id,reason,originating_node,
               failure_class,resume_target,resolver,resolution,resolution_note,resolved_at
             ) values($1::uuid,$2,$3,$4,$5,$6,$7,null,null,null,null)
             returning id::text",
        )
        .bind(process_instance_id)
        .bind(task_id)
        .bind(story_id)
        .bind(reason)
        .bind(originating_node)
        .bind(failure_class)
        .bind(resume_target)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.open_hold", &error))
    }

    pub async fn latest_open_hold(&self, story_id: &str) -> DbResult<Option<ForgeHoldRow>> {
        sqlx::query_as::<_, ForgeHoldRow>(
            "select reason, originating_node
             from forge_hold_record
             where story_id=$1 and resolved_at is null
             order by created_at desc limit 1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.latest_open_hold", &error))
    }

    pub async fn story_packet(&self, story_id: &str) -> DbResult<Option<StoryPacketRow>> {
        sqlx::query_as::<_, StoryPacketRow>(
            "select id, coalesce(title,'') as title, goal, architect_brief,
                    acceptance_criteria, assay_commands
             from storyboard_story where id=$1 limit 1",
        )
        .bind(story_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.story_packet", &error))
    }

    pub async fn current_deal_stage(&self, deal_id: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>("select stage from deal where id=$1::uuid limit 1")
            .bind(deal_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("forge_engine.current_deal_stage", &error))
    }

    pub async fn compare_and_set_deal_stage(
        &self,
        deal_id: &str,
        from: &str,
        to: &str,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            "update deal
             set stage=$3,
                 closed_at=case when $3='closed' then now() else closed_at end,
                 updated_at=now()
             where id=$1::uuid and stage=$2",
        )
        .bind(deal_id)
        .bind(from)
        .bind(to)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.compare_and_set_deal_stage", &error))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_deal_field(&self, deal_id: &str, field: &str, value: &str) -> DbResult<bool> {
        let result = match field {
            "closing_date" => sqlx::query("update deal set closing_date=$2::date,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "inspection_deadline" => sqlx::query("update deal set inspection_deadline=$2::date,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "financing_deadline" => sqlx::query("update deal set financing_deadline=$2::date,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "financing_type" => sqlx::query("update deal set financing_type=$2,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "appraisal_required" => sqlx::query("update deal set appraisal_required=$2::boolean,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            "lender_clear_to_close" => sqlx::query("update deal set lender_clear_to_close=$2::boolean,updated_at=now() where id=$1::uuid")
                .bind(deal_id).bind(value).execute(self.db.pool()).await,
            _ => return Ok(false),
        }
        .map_err(|error| DbFailure::from_sqlx("forge_engine.set_deal_field", &error))?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn vendor_session_id(
        &self,
        story_id: &str,
        worker_id: &str,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, Option<String>>(
            "select session_id from forge_vendor_session
             where story_id=$1 and worker_id=$2 limit 1",
        )
        .bind(story_id)
        .bind(worker_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.vendor_session_id", &error))
        .map(Option::flatten)
    }

    pub async fn write_vendor_session_id(
        &self,
        story_id: &str,
        worker_id: &str,
        session_id: Option<&str>,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into forge_vendor_session(story_id,worker_id,session_id,updated_at)
             values($1,$2,$3,now())
             on conflict(story_id,worker_id)
             do update set session_id=excluded.session_id,updated_at=now()",
        )
        .bind(story_id)
        .bind(worker_id)
        .bind(session_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.write_vendor_session_id", &error))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ForgeEvidencePatch {
    pub work_type: Option<String>,
    pub scout_required: Option<bool>,
    pub lead_decision: Option<String>,
    pub qa_review_required: Option<bool>,
    pub qa_review_passed: Option<bool>,
    pub qa_passed: Option<bool>,
    pub failure_class: Option<String>,
    pub failed_release_stage: Option<String>,
    pub last_failure: Option<String>,
    pub publish_succeeded: Option<bool>,
    pub candidate_sha: Option<String>,
    pub qa_verified_sha: Option<String>,
    pub published_sha: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DealWorkflowFactRow {
    pub stage: Option<String>,
    pub financing_type: Option<String>,
    pub closing_date: Option<String>,
    pub inspection_deadline: Option<String>,
    pub financing_deadline: Option<String>,
    pub appraisal_required: Option<String>,
    pub lender_clear_to_close: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct WorkflowCommandReceiptRow {
    pub outcome: String,
    pub message: Option<String>,
}

impl ForgeEngineDao {
    pub async fn merge_workflow_evidence(
        &self,
        process_instance_id: &str,
        story_id: &str,
        evidence: &ForgeEvidencePatch,
        release_failure_resolved: bool,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into forge_workflow_evidence (
                process_instance_id, story_id, work_type, scout_required, lead_decision,
                qa_review_required, qa_review_passed, qa_passed, failure_class, failed_release_stage,
                last_failure, publish_succeeded, candidate_sha, qa_verified_sha, published_sha
             ) values (
                $1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15
             )
             on conflict (process_instance_id) do update set
                work_type=coalesce(excluded.work_type,forge_workflow_evidence.work_type),
                scout_required=coalesce(excluded.scout_required,forge_workflow_evidence.scout_required),
                lead_decision=coalesce(excluded.lead_decision,forge_workflow_evidence.lead_decision),
                qa_review_required=coalesce(excluded.qa_review_required,forge_workflow_evidence.qa_review_required),
                qa_review_passed=coalesce(excluded.qa_review_passed,forge_workflow_evidence.qa_review_passed),
                qa_passed=coalesce(excluded.qa_passed,forge_workflow_evidence.qa_passed),
                failure_class=case when $16 then null else coalesce(excluded.failure_class,forge_workflow_evidence.failure_class) end,
                failed_release_stage=case when $16 then null else coalesce(excluded.failed_release_stage,forge_workflow_evidence.failed_release_stage) end,
                last_failure=case when $16 then null else coalesce(excluded.last_failure,forge_workflow_evidence.last_failure) end,
                publish_succeeded=coalesce(excluded.publish_succeeded,forge_workflow_evidence.publish_succeeded),
                candidate_sha=coalesce(excluded.candidate_sha,forge_workflow_evidence.candidate_sha),
                qa_verified_sha=coalesce(excluded.qa_verified_sha,forge_workflow_evidence.qa_verified_sha),
                published_sha=coalesce(excluded.published_sha,forge_workflow_evidence.published_sha),
                updated_at=now()",
        )
        .bind(process_instance_id)
        .bind(story_id)
        .bind(evidence.work_type.as_deref())
        .bind(evidence.scout_required)
        .bind(evidence.lead_decision.as_deref())
        .bind(evidence.qa_review_required)
        .bind(evidence.qa_review_passed)
        .bind(evidence.qa_passed)
        .bind(evidence.failure_class.as_deref())
        .bind(evidence.failed_release_stage.as_deref())
        .bind(evidence.last_failure.as_deref())
        .bind(evidence.publish_succeeded)
        .bind(evidence.candidate_sha.as_deref())
        .bind(evidence.qa_verified_sha.as_deref())
        .bind(evidence.published_sha.as_deref())
        .bind(release_failure_resolved)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.merge_workflow_evidence", &error))?;
        Ok(())
    }

    pub async fn record_observer(
        &self,
        event_type: &str,
        summary: &str,
        source_event_id: &str,
        process_instance_id: &str,
        node_id: &str,
        task_id: &str,
        story_id: &str,
    ) -> DbResult<()> {
        sqlx::query(
            "insert into workflow_execution_trace_event (
                event_type, system, occurred_at, outcome, summary,
                source_system, source_event_id,
                workflow_instance_id, workflow_node_id, task_id, correlation_id
             ) values (
                $1,'forge_observer',now(),'ok',$2,
                'forge_observer',$3,$4,$5,$6,$7
             )
             on conflict (source_system, source_event_id)
             where source_event_id is not null do nothing",
        )
        .bind(event_type)
        .bind(summary)
        .bind(source_event_id)
        .bind(process_instance_id)
        .bind(node_id)
        .bind(task_id)
        .bind(story_id)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.record_observer", &error))?;
        Ok(())
    }

    pub async fn deal_id_for_contract(&self, contract_id: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "select f.deal_id::text
             from contract c
             left join document_form_instance f on f.id=c.source_form_instance_id
             where c.id=$1::uuid
             limit 1",
        )
        .bind(contract_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.deal_id_for_contract", &error))
    }

    pub async fn deal_workflow_facts(
        &self,
        deal_id: &str,
    ) -> DbResult<Option<DealWorkflowFactRow>> {
        sqlx::query_as::<_, DealWorkflowFactRow>(
            "select
                d.stage,
                nullif(d.financing_type,'') as financing_type,
                d.closing_date::text as closing_date,
                d.inspection_deadline::text as inspection_deadline,
                d.financing_deadline::text as financing_deadline,
                d.appraisal_required::text as appraisal_required,
                d.lender_clear_to_close::text as lender_clear_to_close
             from deal d
             where d.id=$1::uuid
             limit 1",
        )
        .bind(deal_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.deal_workflow_facts", &error))
    }

    pub async fn closing_document_counts(&self, deal_id: &str) -> DbResult<(i64, i64)> {
        sqlx::query_as::<_, (i64, i64)>(
            "select
                count(*) filter (where status not in ('signed','final'))::bigint,
                count(*)::bigint
             from transaction_document
             where deal_id=$1::uuid",
        )
        .bind(deal_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.closing_document_counts", &error))
    }

    pub async fn claim_workflow_receipt(
        &self,
        command_id: &str,
        actor: Option<&str>,
    ) -> DbResult<Option<WorkflowCommandReceiptRow>> {
        let inserted = sqlx::query_scalar::<_, String>(
            "insert into workflow_command_receipt
             (command_id,outcome,aggregate_id,message,actor_app_user_id)
             values($1,'pending',null,null,$2::uuid)
             on conflict(command_id) do nothing
             returning command_id",
        )
        .bind(command_id)
        .bind(actor)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.claim_workflow_receipt", &error))?;
        if inserted.is_some() {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, WorkflowCommandReceiptRow>(
            "select outcome, message
             from workflow_command_receipt
             where command_id=$1
             limit 1",
        )
        .bind(command_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.read_workflow_receipt", &error))?;
        Ok(row.filter(|row| row.outcome != "pending"))
    }

    pub async fn finalize_workflow_receipt(
        &self,
        command_id: &str,
        outcome: &str,
        aggregate_id: Option<&str>,
        message: Option<&str>,
    ) -> DbResult<()> {
        sqlx::query(
            "update workflow_command_receipt
             set outcome=$2, aggregate_id=$3::uuid, message=$4
             where command_id=$1",
        )
        .bind(command_id)
        .bind(outcome)
        .bind(aggregate_id)
        .bind(message)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("forge_engine.finalize_workflow_receipt", &error))?;
        Ok(())
    }
}
