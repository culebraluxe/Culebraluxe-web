use crate::{Database, DbFailure, DbResult};
use domain::{
    SupportSecurityStatus, SupportSystemHealth, WorkflowAnomaly, WorkflowDefinitionSummary,
    WorkflowDiagnosticsDetail, WorkflowDiagnosticsSnapshot, WorkflowDiagnosticsSummary,
    WorkflowInstanceSummary,
};
use serde_json::Value;
use sqlx::{types::Json, FromRow};
use std::collections::BTreeMap;

#[derive(Debug, FromRow)]
struct SecurityBaseRow {
    active_internal_users: i64,
    external_users: i64,
    has_roles: bool,
    has_identities: bool,
}

#[derive(Debug, FromRow)]
struct SecurityRoleRow {
    users_with_no_role: i64,
    users_with_multiple_roles: i64,
    owner_role_assignments: i64,
    inactive_users_with_active_role_mappings: i64,
    account_type_mismatch_count: i64,
}

#[derive(Debug, FromRow)]
struct SecurityIdentityRow {
    mapped_auth_identities: i64,
    unmapped_app_users: i64,
}

#[derive(Debug, FromRow)]
struct SystemBaseRow {
    unresolved_intake_count: i64,
    open_task_count: i64,
    overdue_task_count: i64,
    active_deal_count: i64,
    under_contract_count: i64,
    active_property_count: i64,
    recent_interaction_at_label: Option<String>,
    interactions_last7_days: i64,
    persons_without_email_identity: i64,
    persons_without_phone_identity: i64,
    open_tasks_without_due_date: i64,
    active_properties_without_hero_media: i64,
    completed_showings_missing_completed_at: i64,
    scheduled_showings_missing_scheduled_at: i64,
    active_participants_with_ended_at: i64,
    other_participants_missing_role_label: i64,
    offers_with_cross_deal_parent: i64,
    showings_with_deal_property_mismatch: i64,
    completed_showings_missing_showing_interaction: i64,
    inactive_participants_without_ended_at: i64,
    public_properties_with_multiple_heroes: i64,
    hero_media_not_image: i64,
    has_roles: bool,
    has_identities: bool,
}

#[derive(Debug, FromRow)]
struct SystemRoleRow {
    account_type_mismatch_count: i64,
    active_app_users_without_role: i64,
    owner_assignments: i64,
}

#[derive(Debug, FromRow)]
struct SystemIdentityRow {
    auth_identity_inactive_app_user: i64,
    auth_identity_without_usable_app_user: i64,
}

#[derive(Debug, FromRow)]
struct DefinitionRow {
    definition_id: String,
    key: String,
    version: i32,
    name: String,
    status: String,
    instance_count: i64,
    active_count: i64,
}

#[derive(Debug, FromRow)]
struct InstanceRow {
    instance_id: String,
    definition_key: String,
    definition_version: i32,
    subject_type: Option<String>,
    subject_id: Option<String>,
    status: String,
    outcome: Option<String>,
    started_at: String,
    ended_at: Option<String>,
    active_token_count: i64,
    task_count: i64,
    event_count: i64,
    property_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct DiagnosticsCountsRow {
    ready_engine_tasks: i64,
    correlated_open_canonical_tasks: i64,
    pending_jobs: i64,
    pending_receipts: i64,
}

#[derive(Debug, FromRow)]
struct AnomalyRow {
    kind: String,
    severity: String,
    instance_id: Option<String>,
    subject_id: Option<String>,
    message: String,
}

#[derive(Debug, FromRow)]
struct DetailRow {
    variables: Option<Value>,
    definition: Value,
    tokens: Json<Vec<Value>>,
    tasks: Json<Vec<Value>>,
    jobs: Json<Vec<Value>>,
    events: Json<Vec<Value>>,
    correlations: Json<Vec<Value>>,
    commands: Json<Vec<Value>>,
}

#[derive(Debug, FromRow)]
struct BreakGlassProbeRow {
    root_resolvable: bool,
    root_active: bool,
    owner_role_present: bool,
    audit_table_available: bool,
}

#[derive(Clone)]
pub struct SupportDiagnosticsDao {
    db: Database,
}

impl SupportDiagnosticsDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn db_diagnostic_counts(&self) -> DbResult<(i64, i64)> {
        let directory_count =
            sqlx::query_scalar::<_, i64>("select count(*)::bigint from mv_client_directory")
                .fetch_one(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("diagnostics.directory_count", &error))?;
        let person_count = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from person where archived_at is null",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("diagnostics.person_count", &error))?;
        Ok((directory_count, person_count))
    }

    pub async fn break_glass_probe(
        &self,
        app_user_id: Option<&str>,
    ) -> DbResult<(bool, bool, bool, bool)> {
        let row = sqlx::query_as::<_, BreakGlassProbeRow>(
            r#"
            select
              case when $1::uuid is null then false else exists(
                select 1 from app_user u where u.id=$1::uuid
              ) end as root_resolvable,
              case when $1::uuid is null then false else exists(
                select 1 from app_user u where u.id=$1::uuid and u.active=true
              ) end as root_active,
              case when $1::uuid is null or to_regclass('app_user_role') is null then false else exists(
                select 1
                from app_user_role aur
                join security_role r on r.id=aur.role_id
                where aur.app_user_id=$1::uuid and r.code in ('owner','root')
              ) end as owner_role_present,
              to_regclass('security_audit_event') is not null as audit_table_available
            "#,
        )
        .bind(app_user_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.break_glass_probe", &error))?;
        Ok((
            row.root_resolvable,
            row.root_active,
            row.owner_role_present,
            row.audit_table_available,
        ))
    }

    pub async fn security_status(&self) -> DbResult<SupportSecurityStatus> {
        let base = sqlx::query_as::<_, SecurityBaseRow>(
            r#"
            select
              (select count(*)::bigint from app_user where account_type='internal' and active=true) as active_internal_users,
              (select count(*)::bigint from app_user where account_type='external') as external_users,
              to_regclass('app_user_role') is not null as has_roles,
              to_regclass('auth_identity') is not null as has_identities
            "#,
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.security_status.base", &error))?;

        let role = if base.has_roles {
            Some(
                sqlx::query_as::<_, SecurityRoleRow>(
                    r#"
                    select
                      (select count(*)::bigint from app_user u
                       where not exists (select 1 from app_user_role aur where aur.app_user_id=u.id)) as users_with_no_role,
                      (select count(*)::bigint from (
                         select app_user_id from app_user_role group by app_user_id having count(*) > 1
                       ) multi) as users_with_multiple_roles,
                      (select count(*)::bigint from app_user_role aur
                       join security_role r on r.id=aur.role_id where r.code='owner') as owner_role_assignments,
                      (select count(*)::bigint from app_user_role aur
                       join app_user u on u.id=aur.app_user_id
                       join security_role r on r.id=aur.role_id
                       where u.active=false and r.active=true) as inactive_users_with_active_role_mappings,
                      (select count(*)::bigint from app_user_role aur
                       join security_role r on r.id=aur.role_id
                       join app_user u on u.id=aur.app_user_id
                       where r.account_type<>u.account_type) as account_type_mismatch_count
                    "#,
                )
                .fetch_one(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("support.security_status.roles", &error))?,
            )
        } else {
            None
        };

        let identity = if base.has_identities {
            Some(
                sqlx::query_as::<_, SecurityIdentityRow>(
                    r#"
                    select
                      (select count(*)::bigint from auth_identity) as mapped_auth_identities,
                      (select count(*)::bigint from app_user u
                       where not exists (select 1 from auth_identity ai where ai.app_user_id=u.id)) as unmapped_app_users
                    "#,
                )
                .fetch_one(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("support.security_status.identities", &error))?,
            )
        } else {
            None
        };

        Ok(SupportSecurityStatus {
            active_internal_users: base.active_internal_users,
            external_users: base.external_users,
            users_with_no_role: role.as_ref().map(|row| row.users_with_no_role).unwrap_or(0),
            users_with_multiple_roles: role
                .as_ref()
                .map(|row| row.users_with_multiple_roles)
                .unwrap_or(0),
            mapped_auth_identities: identity
                .as_ref()
                .map(|row| row.mapped_auth_identities)
                .unwrap_or(0),
            unmapped_app_users: identity
                .as_ref()
                .map(|row| row.unmapped_app_users)
                .unwrap_or(base.active_internal_users),
            owner_role_assignments: role
                .as_ref()
                .map(|row| row.owner_role_assignments)
                .unwrap_or(0),
            inactive_users_with_active_role_mappings: role
                .as_ref()
                .map(|row| row.inactive_users_with_active_role_mappings)
                .unwrap_or(0),
            account_type_mismatch_count: role
                .as_ref()
                .map(|row| row.account_type_mismatch_count)
                .unwrap_or(0),
        })
    }

    pub async fn system_health(&self) -> DbResult<SupportSystemHealth> {
        let base = sqlx::query_as::<_, SystemBaseRow>(
            r#"
            select
              (select count(*)::bigint from website_intake_submission
               where status in ('received','resolution_required')) as unresolved_intake_count,
              (select count(*)::bigint from task where status='open') as open_task_count,
              (select count(*)::bigint from task
               where status='open' and due_at is not null and due_at<now()) as overdue_task_count,
              (select count(*)::bigint from deal where stage<>'closed') as active_deal_count,
              (select count(*)::bigint from deal where stage='under_contract') as under_contract_count,
              (select count(*)::bigint from property
               where archived_at is null and status in ('active','coming_soon','under_contract')) as active_property_count,
              (select to_char(max(occurred_at) at time zone 'America/Puerto_Rico','Mon FMDD, YYYY HH12:MI AM')
               from interaction) as recent_interaction_at_label,
              (select count(*)::bigint from interaction
               where occurred_at>=now()-interval '7 days') as interactions_last7_days,
              (select count(*)::bigint from person p
               where p.archived_at is null
                 and not exists (select 1 from person_identity pi where pi.person_id=p.id and pi.identity_type='email')) as persons_without_email_identity,
              (select count(*)::bigint from person p
               where p.archived_at is null
                 and not exists (select 1 from person_identity pi where pi.person_id=p.id and pi.identity_type='phone')) as persons_without_phone_identity,
              (select count(*)::bigint from task where status='open' and due_at is null) as open_tasks_without_due_date,
              (select count(*)::bigint from property p
               where p.archived_at is null and p.status in ('active','coming_soon','under_contract')
                 and not exists (select 1 from property_media pm where pm.property_id=p.id and pm.role='hero')) as active_properties_without_hero_media,
              (select count(*)::bigint from showing where status='completed' and completed_at is null) as completed_showings_missing_completed_at,
              (select count(*)::bigint from showing where status='scheduled' and scheduled_at is null) as scheduled_showings_missing_scheduled_at,
              (select count(*)::bigint from deal_participant where active=true and ended_at is not null) as active_participants_with_ended_at,
              (select count(*)::bigint from deal_participant where role='other' and role_label is null) as other_participants_missing_role_label,
              (select count(*)::bigint from offer o join offer parent on parent.id=o.parent_offer_id
               where parent.deal_id<>o.deal_id) as offers_with_cross_deal_parent,
              (select count(*)::bigint from showing s join deal d on d.id=s.deal_id
               where s.property_id is not null and s.property_id<>d.property_id) as showings_with_deal_property_mismatch,
              (select count(*)::bigint from showing s
               where s.status='completed'
                 and not exists (select 1 from interaction i
                   where i.source_system='showing' and i.source_external_id=s.id::text)) as completed_showings_missing_showing_interaction,
              (select count(*)::bigint from deal_participant
               where active=false and ended_at is null) as inactive_participants_without_ended_at,
              (select count(*)::bigint from (
                 select pm.property_id from property_media pm where pm.role='hero'
                 group by pm.property_id having count(*)>1
               ) multi_hero join property p on p.id=multi_hero.property_id
               where p.archived_at is null) as public_properties_with_multiple_heroes,
              (select count(*)::bigint from property_media pm join media m on m.id=pm.media_id
               where pm.role='hero' and m.media_type<>'image') as hero_media_not_image,
              to_regclass('app_user_role') is not null as has_roles,
              to_regclass('auth_identity') is not null as has_identities
            "#,
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.system_health.base", &error))?;

        let role = if base.has_roles {
            Some(
                sqlx::query_as::<_, SystemRoleRow>(
                    r#"
                    select
                      (select count(*)::bigint from app_user_role aur
                       join security_role r on r.id=aur.role_id
                       join app_user u on u.id=aur.app_user_id
                       where r.account_type<>u.account_type) as account_type_mismatch_count,
                      (select count(*)::bigint from app_user u where u.active=true
                       and not exists (select 1 from app_user_role aur where aur.app_user_id=u.id)) as active_app_users_without_role,
                      (select count(*)::bigint from app_user_role aur
                       join security_role r on r.id=aur.role_id where r.code='owner') as owner_assignments
                    "#,
                )
                .fetch_one(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("support.system_health.roles", &error))?,
            )
        } else {
            None
        };

        let identity = if base.has_identities {
            Some(
                sqlx::query_as::<_, SystemIdentityRow>(
                    r#"
                    select
                      (select count(*)::bigint from auth_identity ai
                       join app_user u on u.id=ai.app_user_id where u.active=false) as auth_identity_inactive_app_user,
                      (select count(*)::bigint from auth_identity ai
                       where not exists (select 1 from app_user u where u.id=ai.app_user_id and u.active=true)) as auth_identity_without_usable_app_user
                    "#,
                )
                .fetch_one(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx("support.system_health.identities", &error))?,
            )
        } else {
            None
        };

        let owner_assignments = role.as_ref().map(|row| row.owner_assignments).unwrap_or(0);
        Ok(SupportSystemHealth {
            unresolved_intake_count: base.unresolved_intake_count,
            open_task_count: base.open_task_count,
            overdue_task_count: base.overdue_task_count,
            active_deal_count: base.active_deal_count,
            under_contract_count: base.under_contract_count,
            active_property_count: base.active_property_count,
            recent_interaction_at_label: base.recent_interaction_at_label,
            interactions_last7_days: base.interactions_last7_days,
            persons_without_email_identity: base.persons_without_email_identity,
            persons_without_phone_identity: base.persons_without_phone_identity,
            open_tasks_without_due_date: base.open_tasks_without_due_date,
            active_properties_without_hero_media: base.active_properties_without_hero_media,
            completed_showings_missing_completed_at: base.completed_showings_missing_completed_at,
            scheduled_showings_missing_scheduled_at: base.scheduled_showings_missing_scheduled_at,
            active_participants_with_ended_at: base.active_participants_with_ended_at,
            other_participants_missing_role_label: base.other_participants_missing_role_label,
            offers_with_cross_deal_parent: base.offers_with_cross_deal_parent,
            showings_with_deal_property_mismatch: base.showings_with_deal_property_mismatch,
            completed_showings_missing_showing_interaction: base
                .completed_showings_missing_showing_interaction,
            inactive_participants_without_ended_at: base.inactive_participants_without_ended_at,
            public_properties_with_multiple_heroes: base.public_properties_with_multiple_heroes,
            hero_media_not_image: base.hero_media_not_image,
            account_type_mismatch_count: role
                .as_ref()
                .map(|row| row.account_type_mismatch_count)
                .unwrap_or(0),
            active_app_users_without_role: role
                .as_ref()
                .map(|row| row.active_app_users_without_role)
                .unwrap_or(0),
            auth_identity_inactive_app_user: identity
                .as_ref()
                .map(|row| row.auth_identity_inactive_app_user)
                .unwrap_or(0),
            owner_assignments,
            multiple_owners: if owner_assignments > 1 { 1 } else { 0 },
            auth_identity_without_usable_app_user: identity
                .as_ref()
                .map(|row| row.auth_identity_without_usable_app_user)
                .unwrap_or(0),
        })
    }

    pub async fn workflow_diagnostics(&self) -> DbResult<WorkflowDiagnosticsSnapshot> {
        let definitions = sqlx::query_as::<_, DefinitionRow>(
            r#"
            select d.id::text as definition_id,d.key,d.version,d.name,d.status,
                   count(pi.id)::bigint as instance_count,
                   count(pi.id) filter(where pi.status='active')::bigint as active_count
            from process_definitions d
            left join process_instances pi on pi.definition_id=d.id
            group by d.id,d.key,d.version,d.name,d.status
            order by d.key,d.version
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.workflow.definitions", &error))?;

        let instances = sqlx::query_as::<_, InstanceRow>(
            r#"
            select
              pi.id::text as instance_id,
              pd.key as definition_key,
              pd.version as definition_version,
              pi.subject_type,
              pi.subject_id,
              pi.status,
              pi.outcome,
              pi.started_at::text as started_at,
              pi.ended_at::text as ended_at,
              (select count(*)::bigint from tokens t where t.process_instance_id=pi.id and t.status='active') as active_token_count,
              (select count(*)::bigint from tasks t where t.process_instance_id=pi.id) as task_count,
              (select count(*)::bigint from process_events e where e.process_instance_id=pi.id) as event_count,
              p.name as property_name
            from process_instances pi
            join process_definitions pd on pd.id=pi.definition_id
            left join deal d on pi.subject_type='deal'
              and pi.subject_id ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
              and d.id=pi.subject_id::uuid
            left join property p on p.id=d.property_id
            order by pi.started_at desc
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.workflow.instances", &error))?;

        let counts = sqlx::query_as::<_, DiagnosticsCountsRow>(
            r#"
            select
              (select count(*)::bigint from tasks where status in ('ready','reserved','in_progress')) as ready_engine_tasks,
              (select count(distinct c.application_task_id)::bigint
               from workflow_task_correlation c join task t on t.id=c.application_task_id
               where t.status='open') as correlated_open_canonical_tasks,
              (select count(*)::bigint from jobs where status in ('pending','locked')) as pending_jobs,
              (select count(*)::bigint from workflow_command_receipt where outcome='pending') as pending_receipts
            "#,
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.workflow.counts", &error))?;

        let anomalies = self.workflow_anomalies().await?;
        let instance_active = instances
            .iter()
            .filter(|row| row.status == "active")
            .count() as i64;
        let instance_completed = instances
            .iter()
            .filter(|row| row.status == "completed")
            .count() as i64;
        let instance_failed = instances
            .iter()
            .filter(|row| row.status == "error" || row.outcome.as_deref() == Some("failed"))
            .count() as i64;
        let instance_total = instances.len() as i64;

        Ok(WorkflowDiagnosticsSnapshot {
            configured: true,
            summary: WorkflowDiagnosticsSummary {
                definition_count: definitions.len() as i64,
                instance_total,
                instance_active,
                instance_completed,
                instance_failed,
                instance_other: instance_total
                    - instance_active
                    - instance_completed
                    - instance_failed,
                ready_engine_tasks: counts.ready_engine_tasks,
                correlated_open_canonical_tasks: counts.correlated_open_canonical_tasks,
                pending_jobs: counts.pending_jobs,
                pending_receipts: counts.pending_receipts,
                anomaly_count: anomalies.len() as i64,
            },
            definitions: definitions.into_iter().map(map_definition).collect(),
            instances: instances.into_iter().map(map_instance).collect(),
            anomalies,
        })
    }

    async fn workflow_anomalies(&self) -> DbResult<Vec<WorkflowAnomaly>> {
        let rows = sqlx::query_as::<_, AnomalyRow>(
            r#"
            select * from (
              select 'failed-process'::text kind,'critical'::text severity,pi.id::text instance_id,pi.subject_id,
                case when pi.status='error'
                  then format('Process %s is in error state (%s v%s)',pi.id,pd.key,pd.version)
                  else format('Process %s terminated with outcome %s (%s v%s)',pi.id,coalesce(pi.outcome,''),pd.key,pd.version)
                end message
              from process_instances pi join process_definitions pd on pd.id=pi.definition_id
              where pi.status='error' or pi.outcome='failed'

              union all
              select 'pending-receipt','critical',null,null,
                format('Command receipt %s is stuck pending%s',command_id,
                  case when message is null then '' else ' — '||message end)
              from workflow_command_receipt where outcome='pending'

              union all
              select 'ready-task-uncorrelated','warning',t.process_instance_id::text,null,
                format('Engine task %s (%s) is %s but has no canonical correlation',t.name,t.id,t.status)
              from tasks t left join workflow_task_correlation c on c.workflow_task_id=t.id::text
              where t.status in ('ready','reserved','in_progress') and c.workflow_task_id is null

              union all
              select 'correlation-dangling-app-task','warning',null,c.application_task_id::text,
                format('Correlation %s references missing canonical task %s',c.workflow_task_id,c.application_task_id)
              from workflow_task_correlation c left join task t on t.id=c.application_task_id where t.id is null

              union all
              select 'correlation-dangling-workflow-task','warning',null,null,
                format('Correlation %s references a missing engine task (canonical %s)',c.workflow_task_id,c.application_task_id)
              from workflow_task_correlation c left join tasks t on t.id::text=c.workflow_task_id where t.id is null

              union all
              select 'open-job-on-closed-token','warning',j.process_instance_id::text,null,
                format('Job %s (%s) is %s but token %s is already %s',j.id,j.type,j.status,t.id,t.outcome)
              from jobs j join tokens t on t.id=j.token_id
              where j.status in ('pending','locked') and t.outcome in ('completed','skipped')

              union all
              select 'multiple-active-instances','warning',null,subject_id,
                format('Subject %s:%s has %s active instances',subject_type,subject_id,count(*))
              from process_instances where status='active' and subject_id is not null
              group by subject_type,subject_id having count(*)>1

              union all
              select 'stale-locked-job','warning',j.process_instance_id::text,null,
                format('Job %s (%s) is locked by %s past its lease (%s)',j.id,j.type,j.locked_by,j.locked_until)
              from jobs j where j.status='locked' and j.locked_until<now()

              union all
              select 'wedged-instance','warning',pi.id::text,null,
                format('Process %s (%s v%s) is active but has no active tokens or pending work — was never terminalized',pi.id,pd.key,pd.version)
              from process_instances pi join process_definitions pd on pd.id=pi.definition_id
              where pi.status='active'
                and not exists(select 1 from tokens t where t.process_instance_id=pi.id and t.status='active')
                and not exists(select 1 from jobs j where j.process_instance_id=pi.id and j.status in ('pending','locked'))

              union all
              select 'orphan-token','warning',t.process_instance_id::text,null,
                format('Token %s at %s is active but its process is missing or not active',t.id,t.node_id)
              from tokens t left join process_instances pi on pi.id=t.process_instance_id
              where t.status='active' and (pi.id is null or pi.status<>'active')

              union all
              select 'error-instance-missing-outcome','critical',pi.id::text,null,
                format('Process %s (%s v%s) is error with no terminal outcome',pi.id,pd.key,pd.version)
              from process_instances pi join process_definitions pd on pd.id=pi.definition_id
              where pi.status='error' and pi.outcome is null
            ) anomaly
            order by severity,kind,instance_id nulls last
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.workflow.anomalies", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| WorkflowAnomaly {
                kind: row.kind,
                severity: row.severity,
                instance_id: row.instance_id,
                subject_id: row.subject_id,
                message: row.message,
            })
            .collect())
    }

    pub async fn workflow_detail(
        &self,
        instance_id: &str,
    ) -> DbResult<Option<WorkflowDiagnosticsDetail>> {
        let summary = sqlx::query_as::<_, InstanceRow>(
            r#"
            select
              pi.id::text as instance_id,pd.key as definition_key,pd.version as definition_version,
              pi.subject_type,pi.subject_id,pi.status,pi.outcome,
              pi.started_at::text as started_at,pi.ended_at::text as ended_at,
              (select count(*)::bigint from tokens t where t.process_instance_id=pi.id and t.status='active') active_token_count,
              (select count(*)::bigint from tasks t where t.process_instance_id=pi.id) task_count,
              (select count(*)::bigint from process_events e where e.process_instance_id=pi.id) event_count,
              p.name as property_name
            from process_instances pi
            join process_definitions pd on pd.id=pi.definition_id
            left join deal d on pi.subject_type='deal'
              and pi.subject_id ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
              and d.id=pi.subject_id::uuid
            left join property p on p.id=d.property_id
            where pi.id=$1::uuid
            "#,
        )
        .bind(instance_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.workflow.detail.summary", &error))?;
        let Some(summary) = summary else {
            return Ok(None);
        };

        let detail = sqlx::query_as::<_, DetailRow>(
            r#"
            select
              pi.variables,
              pd.definition,
              coalesce((select jsonb_agg(jsonb_build_object(
                'id',t.id::text,'parentTokenId',t.parent_token_id::text,'nodeId',t.node_id,
                'status',t.status,'outcome',t.outcome,'required',t.required
              ) order by t.created_at,t.id) from tokens t where t.process_instance_id=pi.id),'[]'::jsonb) as tokens,
              coalesce((select jsonb_agg(jsonb_build_object(
                'id',t.id::text,'tokenId',t.token_id::text,'name',t.name,'status',t.status,
                'candidates',t.candidates,'assignee',t.assignee
              ) order by t.created_at) from tasks t where t.process_instance_id=pi.id),'[]'::jsonb) as tasks,
              coalesce((select jsonb_agg(jsonb_build_object(
                'id',j.id::text,'type',j.type,'status',j.status,'dueAt',j.due_at::text
              ) order by j.created_at) from jobs j where j.process_instance_id=pi.id),'[]'::jsonb) as jobs,
              coalesce((select jsonb_agg(jsonb_build_object(
                'id',e.id::text,'eventType',e.event_type,'nodeId',e.node_id,'actor',e.actor
              ) order by e.id) from process_events e where e.process_instance_id=pi.id),'[]'::jsonb) as events,
              coalesce((select jsonb_agg(jsonb_build_object(
                'workflowTaskId',c.workflow_task_id,'applicationTaskId',c.application_task_id::text,
                'applicationTaskStatus',a.status,'applicationTaskTitle',a.title
              )) from workflow_task_correlation c
                 left join task a on a.id=c.application_task_id
               where c.workflow_task_id in (
                 select t.id::text from tasks t where t.process_instance_id=pi.id
               )),'[]'::jsonb) as correlations,
              coalesce((select jsonb_agg(jsonb_build_object(
                'commandId',pc.command_id,'commandType',pc.command_type,'nodeId',pc.node_id,
                'outcome',pc.outcome,'message',pc.message,'receiptOutcome',r.outcome
              ) order by pc.id)
               from process_commands pc
               left join workflow_command_receipt r on r.command_id=pc.command_id
               where pc.process_instance_id=pi.id),'[]'::jsonb) as commands
            from process_instances pi
            join process_definitions pd on pd.id=pi.definition_id
            where pi.id=$1::uuid
            "#,
        )
        .bind(instance_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("support.workflow.detail", &error))?;

        let mut node_labels = BTreeMap::new();
        if let Some(nodes) = detail.definition.get("nodes").and_then(Value::as_object) {
            for (id, node) in nodes {
                let name = node
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned();
                node_labels.insert(id.clone(), name);
            }
        }

        Ok(Some(WorkflowDiagnosticsDetail {
            instance: map_instance(summary),
            variables: detail.variables,
            node_labels,
            tokens: detail.tokens.0,
            tasks: detail.tasks.0,
            jobs: detail.jobs.0,
            events: detail.events.0,
            correlations: detail.correlations.0,
            commands: detail.commands.0,
        }))
    }
}

fn map_definition(row: DefinitionRow) -> WorkflowDefinitionSummary {
    WorkflowDefinitionSummary {
        definition_id: row.definition_id,
        key: row.key,
        version: row.version,
        name: row.name,
        status: row.status,
        instance_count: row.instance_count,
        active_count: row.active_count,
    }
}

fn map_instance(row: InstanceRow) -> WorkflowInstanceSummary {
    WorkflowInstanceSummary {
        instance_id: row.instance_id,
        definition_key: row.definition_key,
        definition_version: row.definition_version,
        subject_type: row.subject_type,
        subject_id: row.subject_id,
        status: row.status,
        outcome: row.outcome,
        started_at: row.started_at,
        ended_at: row.ended_at,
        active_token_count: row.active_token_count,
        task_count: row.task_count,
        event_count: row.event_count,
        property_name: row.property_name,
    }
}
