use domain::{
    CreateDealRequest, CreateDealResult, DealContractPortfolioItem, DealOwnerCandidate,
    DealPortfolioItem, DealPortfolioSnapshot, DealWorkspaceActivity, DealWorkspaceClient,
    DealWorkspaceCommand, DealWorkspaceCommandResult, DealWorkspaceDeal, DealWorkspaceOffer,
    DealWorkspaceParticipant, DealWorkspaceProperty, DealWorkspaceShowing, DealWorkspaceSnapshot,
    DealWorkspaceTask, DealableProperty,
};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{Database, DbFailure, DbResult};

#[derive(Debug, FromRow)]
struct DealRow {
    id: String,
    property_id: String,
    property_name: String,
    property_location: Option<String>,
    property_type: Option<String>,
    bedrooms: Option<String>,
    hero_media_id: Option<String>,
    client_id: String,
    client_name: String,
    stage: String,
    list_price: Option<String>,
    offer_price: Option<String>,
    owner_name: Option<String>,
    closing_date: Option<String>,
    showing_count: i64,
    offer_count: i64,
    participant_count: i64,
    latest_offer_amount: Option<String>,
    latest_offer_status: Option<String>,
    next_milestone: Option<String>,
    next_milestone_at: Option<String>,
    last_activity: Option<String>,
    last_activity_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct ContractRow {
    id: String,
    form_template_id: String,
    contract_type: String,
    property_id: String,
    property_label: Option<String>,
    status: String,
    process_instance_id: Option<String>,
    executed_at: Option<String>,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct PropertyRow {
    id: String,
    name: Option<String>,
    location: Option<String>,
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: String,
    display_name: String,
    email: Option<String>,
}

#[derive(Debug, FromRow)]
struct WorkspaceHeaderRow {
    deal_id: String,
    stage: String,
    list_price: Option<String>,
    offer_price: Option<String>,
    closing_date_label: Option<String>,
    closed_at_label: Option<String>,
    notes: Option<String>,
    created_at_label: String,
    updated_at_label: String,
    property_id: String,
    property_name: String,
    property_location: Option<String>,
    property_type: Option<String>,
    bedrooms: Option<String>,
    bathrooms: Option<String>,
    square_feet: Option<i64>,
    client_id: String,
    client_name: String,
    client_role: String,
    client_status: String,
    client_email: Option<String>,
    client_phone: Option<String>,
}

#[derive(Debug, FromRow)]
struct WorkspaceTaskRow {
    id: String,
    title: String,
    detail: Option<String>,
    due_at_label: Option<String>,
    is_overdue: bool,
}

#[derive(Debug, FromRow)]
struct WorkspaceActivityRow {
    id: String,
    person_id: Option<String>,
    channel: String,
    direction: Option<String>,
    occurred_at_label: String,
    title: Option<String>,
    summary: Option<String>,
    person_name: Option<String>,
}

#[derive(Debug, FromRow)]
struct WorkspaceParticipantRow {
    id: String,
    role_category: String,
    role_label: Option<String>,
    person_id: Option<String>,
    user_id: Option<String>,
    person_name: Option<String>,
    user_name: Option<String>,
    person_email: Option<String>,
    person_phone: Option<String>,
}

#[derive(Debug, FromRow)]
struct WorkspaceOfferRow {
    id: String,
    person_id: String,
    person_name: Option<String>,
    parent_offer_id: Option<String>,
    amount: String,
    status: String,
    submitted_at_label: String,
    responded_at_label: Option<String>,
    note: Option<String>,
}

#[derive(Debug, FromRow)]
struct WorkspaceShowingRow {
    id: String,
    person_id: String,
    person_name: String,
    status: String,
    requested_at_label: String,
    scheduled_at_label: Option<String>,
    completed_at_label: Option<String>,
    cancelled_at_label: Option<String>,
    feedback: Option<String>,
}

#[derive(Clone)]
pub struct DealPortalDao {
    db: Database,
}

impl DealPortalDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn portfolio(&self) -> DbResult<DealPortfolioSnapshot> {
        let (deals, contracts, properties, users) = tokio::try_join!(
            self.deals(),
            self.contracts(),
            self.dealable_properties(),
            self.owner_candidates(),
        )?;
        Ok(DealPortfolioSnapshot {
            deals,
            contracts,
            properties,
            users,
        })
    }

    pub async fn create(&self, request: &CreateDealRequest) -> DbResult<CreateDealResult> {
        let id = Uuid::new_v4().to_string();
        let mut tx = self.db.begin("deal.create").await?;
        let result = async {
            let property_active = sqlx::query_scalar::<_, bool>(
                r#"
                select exists(
                  select 1 from property
                  where id=$1::uuid and archived_at is null
                )
                "#,
            )
            .bind(&request.property_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.create.property", &error))?;
            if !property_active {
                return Err(DbFailure::schema_mismatch(
                    "deal.create",
                    "Property not found or archived.",
                ));
            }

            let client_active = sqlx::query_scalar::<_, bool>(
                r#"
                select exists(
                  select 1 from person
                  where id=$1::uuid and archived_at is null
                )
                "#,
            )
            .bind(&request.client_person_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.create.client", &error))?;
            if !client_active {
                return Err(DbFailure::schema_mismatch(
                    "deal.create",
                    "Client person not found or archived.",
                ));
            }

            if let Some(owner_id) = request.owner_user_id.as_deref() {
                let owner_active = sqlx::query_scalar::<_, bool>(
                    "select exists(select 1 from app_user where id=$1::uuid and active=true)",
                )
                .bind(owner_id)
                .fetch_one(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("deal.create.owner", &error))?;
                if !owner_active {
                    return Err(DbFailure::schema_mismatch(
                        "deal.create",
                        "Owner user not found or inactive.",
                    ));
                }
            }

            sqlx::query(
                r#"
                insert into deal (
                  id, property_id, client_person_id, owner_user_id, notes
                )
                values ($1::uuid,$2::uuid,$3::uuid,$4::uuid,$5)
                "#,
            )
            .bind(&id)
            .bind(&request.property_id)
            .bind(&request.client_person_id)
            .bind(request.owner_user_id.as_deref())
            .bind(request.notes.as_deref().map(str::trim).filter(|value| !value.is_empty()))
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.create.insert", &error))?;

            sqlx::query(
                r#"
                insert into deal_participant (deal_id, person_id, role, active)
                values ($1::uuid,$2::uuid,'client',true)
                "#,
            )
            .bind(&id)
            .bind(&request.client_person_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.create.client_participant", &error))?;

            if let Some(owner_id) = request.owner_user_id.as_deref() {
                sqlx::query(
                    r#"
                    insert into deal_participant (deal_id, user_id, role, active)
                    values ($1::uuid,$2::uuid,'owner',true)
                    "#,
                )
                .bind(&id)
                .bind(owner_id)
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("deal.create.owner_participant", &error))?;
            }

            Ok(CreateDealResult { id: id.clone() })
        }
        .await;

        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn command(
        &self,
        deal_id: &str,
        command: &DealWorkspaceCommand,
    ) -> DbResult<DealWorkspaceCommandResult> {
        let mut tx = self.db.begin("deal.workspace.command").await?;
        let result = async {
            let deal_exists = sqlx::query_scalar::<_, bool>(
                "select exists(select 1 from deal where id=$1::uuid)",
            )
            .bind(deal_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.command.deal", &error))?;
            if !deal_exists {
                return Err(DbFailure::schema_mismatch(
                    "deal.workspace.command",
                    "Deal not found.",
                ));
            }

            let id = match command {
                DealWorkspaceCommand::CreateTask {
                    title,
                    detail,
                    due_at,
                } => {
                    let title = title.trim();
                    if title.is_empty() {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.task.create",
                            "Task title is required.",
                        ));
                    }
                    sqlx::query_scalar::<_, String>(
                        r#"
                        insert into task (
                          title, detail, person_id, deal_id, due_at, task_kind, priority
                        )
                        select
                          $2,
                          nullif(btrim($3), ''),
                          client.person_id,
                          d.id,
                          case
                            when nullif(btrim($4), '') is null then null
                            else ($4::timestamp at time zone 'America/Puerto_Rico')
                          end,
                          'human',
                          0
                        from deal d
                        join lateral (
                          select dp.person_id
                          from deal_participant dp
                          where dp.deal_id=d.id
                            and dp.role='client'
                            and dp.active=true
                          order by dp.created_at asc
                          limit 1
                        ) client on true
                        where d.id=$1::uuid
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(title)
                    .bind(detail.as_deref().unwrap_or(""))
                    .bind(due_at.as_deref().unwrap_or(""))
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.task.create", &error))?
                }
                DealWorkspaceCommand::CompleteTask { task_id } => {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update task
                        set status='completed',
                            completed_at=now(),
                            updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and status='open'
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(task_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.task.complete", &error))?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.task.complete",
                            "Task not found or already resolved.",
                        )
                    })?
                }
                DealWorkspaceCommand::CreateShowing {
                    person_id,
                    property_id,
                } => {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        insert into showing (
                          person_id, property_id, deal_id, status, requested_at
                        )
                        select
                          $2::uuid,
                          coalesce($3::uuid, d.property_id),
                          d.id,
                          'requested',
                          now()
                        from deal d
                        where d.id=$1::uuid
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(person_id)
                    .bind(property_id.as_deref())
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.showing.create", &error))?
                }
                DealWorkspaceCommand::ScheduleShowing {
                    showing_id,
                    scheduled_at,
                } => {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update showing
                        set status='scheduled',
                            scheduled_at=($3::timestamp at time zone 'America/Puerto_Rico'),
                            updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and status='requested'
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(showing_id)
                    .bind(scheduled_at)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.showing.schedule", &error))?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.showing.schedule",
                            "Showing not found or not in requested state.",
                        )
                    })?
                }
                DealWorkspaceCommand::CancelShowing { showing_id } => {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update showing
                        set status='cancelled',
                            cancelled_at=now(),
                            updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and status in ('requested','scheduled')
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(showing_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.showing.cancel", &error))?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.showing.cancel",
                            "Showing not found or already resolved.",
                        )
                    })?
                }
                DealWorkspaceCommand::CompleteShowing { showing_id } => {
                    let completed = sqlx::query_scalar::<_, String>(
                        r#"
                        with updated as (
                          update showing
                          set status='completed',
                              completed_at=now(),
                              updated_at=now()
                          where id=$2::uuid
                            and deal_id=$1::uuid
                            and status in ('requested','scheduled')
                          returning id, person_id, property_id, deal_id, scheduled_at, completed_at
                        ),
                        emitted as (
                          insert into interaction (
                            person_id, property_id, deal_id, channel, event_type,
                            occurred_at, title, source_system, source_external_id, source_metadata
                          )
                          select
                            u.person_id,
                            u.property_id,
                            u.deal_id,
                            'showing',
                            'showing_completed',
                            coalesce(u.completed_at, u.scheduled_at),
                            'Showing completed',
                            'showing',
                            u.id::text,
                            '{}'::jsonb
                          from updated u
                          on conflict (source_system, source_external_id)
                            where source_system is not null and source_external_id is not null
                          do nothing
                          returning source_external_id
                        )
                        select id::text from updated
                        "#,
                    )
                    .bind(deal_id)
                    .bind(showing_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.showing.complete", &error))?;
                    match completed {
                        Some(id) => id,
                        None => sqlx::query_scalar::<_, String>(
                            r#"
                            select id::text
                            from showing
                            where id=$2::uuid
                              and deal_id=$1::uuid
                              and status='completed'
                            limit 1
                            "#,
                        )
                        .bind(deal_id)
                        .bind(showing_id)
                        .fetch_optional(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx("deal.workspace.showing.complete.read", &error)
                        })?
                        .ok_or_else(|| {
                            DbFailure::schema_mismatch(
                                "deal.workspace.showing.complete",
                                "Showing not found or cannot be completed.",
                            )
                        })?,
                    }
                }
                DealWorkspaceCommand::SubmitOffer {
                    person_id,
                    amount,
                    parent_offer_id,
                } => {
                    let amount = amount.trim().parse::<f64>().map_err(|_| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.offer.submit",
                            "Offer amount must be a positive number.",
                        )
                    })?;
                    if !amount.is_finite() || amount <= 0.0 {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.offer.submit",
                            "Offer amount must be a positive number.",
                        ));
                    }
                    if let Some(parent_id) = parent_offer_id.as_deref() {
                        let parent_matches = sqlx::query_scalar::<_, bool>(
                            r#"
                            select exists(
                              select 1
                              from offer
                              where id=$2::uuid and deal_id=$1::uuid
                            )
                            "#,
                        )
                        .bind(deal_id)
                        .bind(parent_id)
                        .fetch_one(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx("deal.workspace.offer.parent", &error)
                        })?;
                        if !parent_matches {
                            return Err(DbFailure::schema_mismatch(
                                "deal.workspace.offer.submit",
                                "Parent offer must belong to the same deal.",
                            ));
                        }
                    }
                    sqlx::query_scalar::<_, String>(
                        r#"
                        insert into offer (
                          deal_id, person_id, parent_offer_id, amount, status
                        )
                        values ($1::uuid,$2::uuid,$3::uuid,$4,'submitted')
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(person_id)
                    .bind(parent_offer_id.as_deref())
                    .bind(amount)
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.offer.submit", &error))?
                }
                DealWorkspaceCommand::WithdrawOffer { offer_id } => {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update offer
                        set status='withdrawn',
                            responded_at=now(),
                            updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and status='submitted'
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(offer_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.offer.withdraw", &error))?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.offer.withdraw",
                            "Offer not found or not in submitted state.",
                        )
                    })?
                }
                DealWorkspaceCommand::RejectOffer { offer_id } => {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update offer
                        set status='rejected',
                            responded_at=now(),
                            updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and status='submitted'
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(offer_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("deal.workspace.offer.reject", &error))?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.offer.reject",
                            "Offer not found or not in submitted state.",
                        )
                    })?
                }
                DealWorkspaceCommand::AddOtherParticipant {
                    person_id,
                    role_label,
                } => {
                    let role_label = role_label.trim();
                    if role_label.is_empty() || role_label.chars().count() > 120 {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.add",
                            "Role label is required and must be 120 characters or fewer.",
                        ));
                    }
                    let duplicate = sqlx::query_scalar::<_, bool>(
                        r#"
                        select exists(
                          select 1
                          from deal_participant
                          where deal_id=$1::uuid
                            and role='other'
                            and active=true
                            and lower(role_label)=lower($3)
                        )
                        "#,
                    )
                    .bind(deal_id)
                    .bind(person_id)
                    .bind(role_label)
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("deal.workspace.participant.duplicate", &error)
                    })?;
                    if duplicate {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.add",
                            "An active participant with this role already exists.",
                        ));
                    }
                    sqlx::query_scalar::<_, String>(
                        r#"
                        insert into deal_participant (
                          deal_id, person_id, role, role_label, active
                        )
                        values ($1::uuid,$2::uuid,'other',$3,true)
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(person_id)
                    .bind(role_label)
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("deal.workspace.participant.add", &error)
                    })?
                }
                DealWorkspaceCommand::UpdateOtherParticipant {
                    participant_id,
                    role_label,
                } => {
                    let role_label = role_label.trim();
                    if role_label.is_empty() || role_label.chars().count() > 120 {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.update",
                            "Role label is required and must be 120 characters or fewer.",
                        ));
                    }
                    let duplicate = sqlx::query_scalar::<_, bool>(
                        r#"
                        select exists(
                          select 1
                          from deal_participant
                          where deal_id=$1::uuid
                            and role='other'
                            and active=true
                            and lower(role_label)=lower($3)
                            and id<>$2::uuid
                        )
                        "#,
                    )
                    .bind(deal_id)
                    .bind(participant_id)
                    .bind(role_label)
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("deal.workspace.participant.update_duplicate", &error)
                    })?;
                    if duplicate {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.update",
                            "An active participant with this role already exists.",
                        ));
                    }
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update deal_participant
                        set role_label=$3, updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and role='other'
                          and active=true
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(participant_id)
                    .bind(role_label)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("deal.workspace.participant.update", &error)
                    })?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.participant.update",
                            "Participant not found or not role=other.",
                        )
                    })?
                }
                DealWorkspaceCommand::EndOtherParticipant { participant_id } => {
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update deal_participant
                        set active=false, ended_at=now(), updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and role='other'
                          and active=true
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(participant_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("deal.workspace.participant.end", &error)
                    })?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.participant.end",
                            "Participant not found or already ended.",
                        )
                    })?
                }
                DealWorkspaceCommand::SetStructuralParticipant {
                    role,
                    person_id,
                    user_id,
                } => {
                    if !matches!(role.as_str(), "client" | "owner" | "seller") {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.set_structural",
                            "Structural role must be client, owner, or seller.",
                        ));
                    }
                    let is_owner = role == "owner";
                    if is_owner {
                        if person_id.is_some() || user_id.as_deref().unwrap_or("").is_empty() {
                            return Err(DbFailure::schema_mismatch(
                                "deal.workspace.participant.set_structural",
                                "Owner requires exactly one active user.",
                            ));
                        }
                        let active = sqlx::query_scalar::<_, bool>(
                            "select exists(select 1 from app_user where id=$1::uuid and active=true)",
                        )
                        .bind(user_id.as_deref())
                        .fetch_one(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "deal.workspace.participant.set_structural.user",
                                &error,
                            )
                        })?;
                        if !active {
                            return Err(DbFailure::schema_mismatch(
                                "deal.workspace.participant.set_structural",
                                "Owner user not found or inactive.",
                            ));
                        }
                    } else {
                        if user_id.is_some() || person_id.as_deref().unwrap_or("").is_empty() {
                            return Err(DbFailure::schema_mismatch(
                                "deal.workspace.participant.set_structural",
                                "Client and seller require exactly one active person.",
                            ));
                        }
                        let active = sqlx::query_scalar::<_, bool>(
                            r#"
                            select exists(
                              select 1 from person
                              where id=$1::uuid and archived_at is null
                            )
                            "#,
                        )
                        .bind(person_id.as_deref())
                        .fetch_one(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "deal.workspace.participant.set_structural.person",
                                &error,
                            )
                        })?;
                        if !active {
                            return Err(DbFailure::schema_mismatch(
                                "deal.workspace.participant.set_structural",
                                "Person not found or archived.",
                            ));
                        }
                    }

                    sqlx::query(
                        r#"
                        update deal_participant
                        set active=false, ended_at=now(), updated_at=now()
                        where deal_id=$1::uuid and role=$2 and active=true
                        "#,
                    )
                    .bind(deal_id)
                    .bind(role)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx(
                            "deal.workspace.participant.set_structural.end_current",
                            &error,
                        )
                    })?;

                    let participant_id = sqlx::query_scalar::<_, String>(
                        r#"
                        insert into deal_participant (
                          deal_id, person_id, user_id, role, active
                        )
                        values ($1::uuid,$2::uuid,$3::uuid,$4,true)
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(person_id.as_deref())
                    .bind(user_id.as_deref())
                    .bind(role)
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx(
                            "deal.workspace.participant.set_structural.insert",
                            &error,
                        )
                    })?;

                    if role == "client" {
                        sqlx::query(
                            "update deal set client_person_id=$2::uuid, updated_at=now() where id=$1::uuid",
                        )
                        .bind(deal_id)
                        .bind(person_id.as_deref())
                        .execute(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "deal.workspace.participant.set_structural.client_mirror",
                                &error,
                            )
                        })?;
                    } else if role == "owner" {
                        sqlx::query(
                            "update deal set owner_user_id=$2::uuid, updated_at=now() where id=$1::uuid",
                        )
                        .bind(deal_id)
                        .bind(user_id.as_deref())
                        .execute(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "deal.workspace.participant.set_structural.owner_mirror",
                                &error,
                            )
                        })?;
                    }
                    participant_id
                }
                DealWorkspaceCommand::EndStructuralParticipant { participant_id } => {
                    let row = sqlx::query_as::<_, (String, Option<String>)>(
                        r#"
                        select role, user_id::text
                        from deal_participant
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and active=true
                        limit 1
                        "#,
                    )
                    .bind(deal_id)
                    .bind(participant_id)
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx(
                            "deal.workspace.participant.end_structural.read",
                            &error,
                        )
                    })?
                    .ok_or_else(|| {
                        DbFailure::schema_mismatch(
                            "deal.workspace.participant.end_structural",
                            "Participant not found.",
                        )
                    })?;
                    let (role, user_id) = row;
                    if role == "client" {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.end_structural",
                            "A deal must keep a client; replace the client instead.",
                        ));
                    }
                    if role == "other" {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.end_structural",
                            "Long-tail participants use the other-participant command.",
                        ));
                    }
                    if !matches!(role.as_str(), "owner" | "seller") {
                        return Err(DbFailure::schema_mismatch(
                            "deal.workspace.participant.end_structural",
                            "Participant is not a structural role.",
                        ));
                    }
                    sqlx::query_scalar::<_, String>(
                        r#"
                        update deal_participant
                        set active=false, ended_at=now(), updated_at=now()
                        where id=$2::uuid
                          and deal_id=$1::uuid
                          and active=true
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(participant_id)
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx(
                            "deal.workspace.participant.end_structural",
                            &error,
                        )
                    })?;
                    if role == "owner" {
                        sqlx::query(
                            r#"
                            update deal
                            set owner_user_id=null, updated_at=now()
                            where id=$1::uuid
                              and owner_user_id=$2::uuid
                            "#,
                        )
                        .bind(deal_id)
                        .bind(user_id.as_deref())
                        .execute(tx.connection())
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx(
                                "deal.workspace.participant.end_structural.owner_mirror",
                                &error,
                            )
                        })?;
                    }
                    participant_id.clone()
                }
            };

            Ok(DealWorkspaceCommandResult { id })
        }
        .await;

        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn workspace(&self, deal_id: &str) -> DbResult<DealWorkspaceSnapshot> {
        let header = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceHeaderRow>(
                r#"
                select
                  d.id::text as deal_id,
                  d.stage,
                  d.list_price::text as list_price,
                  d.offer_price::text as offer_price,
                  to_char(d.closing_date, 'Mon FMDD, YYYY') as closing_date_label,
                  to_char(
                    d.closed_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY'
                  ) as closed_at_label,
                  d.notes,
                  to_char(
                    d.created_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY'
                  ) as created_at_label,
                  to_char(
                    d.updated_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY'
                  ) as updated_at_label,
                  p.id::text as property_id,
                  coalesce(p.name, 'Property') as property_name,
                  p.location as property_location,
                  p.property_type,
                  p.bedrooms::text as bedrooms,
                  p.bathrooms::text as bathrooms,
                  p.square_feet::bigint as square_feet,
                  client.id::text as client_id,
                  client.display_name as client_name,
                  client.role as client_role,
                  client.status as client_status,
                  client_email.identity_value as client_email,
                  client_phone.identity_value as client_phone
                from deal d
                join property p on p.id=d.property_id
                join lateral (
                  select person.id, person.display_name, person.role, person.status
                  from deal_participant dp
                  join person on person.id=dp.person_id
                  where dp.deal_id=d.id
                    and dp.role='client'
                    and dp.active=true
                  order by dp.created_at asc
                  limit 1
                ) client on true
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=client.id
                    and pi.identity_type='email'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) client_email on true
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=client.id
                    and pi.identity_type='phone'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) client_phone on true
                where d.id=$1::uuid
                limit 1
                "#,
            )
            .bind(deal_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.header", &error))
        })?;

        let Some(header) = header else {
            return Ok(DealWorkspaceSnapshot::default());
        };

        let property_id = header.property_id.clone();
        let (open_tasks, activity, participants, offers, showings, contracts, owner_candidates) =
            tokio::try_join!(
                self.workspace_tasks(deal_id),
                self.workspace_activity(deal_id),
                self.workspace_participants(deal_id),
                self.workspace_offers(deal_id),
                self.workspace_showings(deal_id),
                self.contracts_for_property(&property_id),
                self.owner_candidates(),
            )?;

        Ok(DealWorkspaceSnapshot {
            deal: Some(DealWorkspaceDeal {
                id: header.deal_id,
                stage: header.stage,
                list_price: parse_number(header.list_price.as_deref()),
                offer_price: parse_number(header.offer_price.as_deref()),
                closing_date_label: header.closing_date_label,
                closed_at_label: header.closed_at_label,
                notes: header.notes,
                created_at_label: header.created_at_label,
                updated_at_label: header.updated_at_label,
            }),
            property: Some(DealWorkspaceProperty {
                id: header.property_id,
                name: header.property_name,
                location: header.property_location,
                property_type: header.property_type,
                bedrooms: parse_number(header.bedrooms.as_deref()),
                bathrooms: parse_number(header.bathrooms.as_deref()),
                square_feet: header.square_feet,
            }),
            client: Some(DealWorkspaceClient {
                id: header.client_id,
                display_name: header.client_name,
                role: header.client_role,
                status: header.client_status,
                email: header.client_email,
                phone: header.client_phone,
            }),
            participants,
            open_tasks,
            activity,
            offers,
            showings,
            contracts,
            owner_candidates,
        })
    }

    async fn workspace_tasks(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceTask>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceTaskRow>(
                r#"
                select
                  t.id::text as id,
                  t.title,
                  t.detail,
                  case when t.due_at is not null
                    then to_char(
                      t.due_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as due_at_label,
                  (t.due_at is not null and t.due_at < now()) as is_overdue
                from task t
                where t.deal_id=$1::uuid
                  and t.status='open'
                order by t.due_at asc nulls last, t.created_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.tasks", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceTask {
                id: row.id,
                title: row.title,
                detail: row.detail,
                due_at_label: row.due_at_label,
                is_overdue: row.is_overdue,
            })
            .collect())
    }

    async fn workspace_activity(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceActivity>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceActivityRow>(
                r#"
                select
                  i.id::text as id,
                  person.id::text as person_id,
                  i.channel,
                  i.direction,
                  to_char(
                    i.occurred_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as occurred_at_label,
                  i.title,
                  i.summary,
                  person.display_name as person_name
                from interaction i
                left join person on person.id=i.person_id
                where i.deal_id=$1::uuid
                order by i.occurred_at desc
                limit 20
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.activity", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceActivity {
                id: row.id,
                person_id: row.person_id,
                channel: row.channel,
                direction: row.direction,
                occurred_at_label: row.occurred_at_label,
                title: row.title,
                summary: row.summary,
                person_name: row.person_name,
            })
            .collect())
    }

    async fn workspace_participants(
        &self,
        deal_id: &str,
    ) -> DbResult<Vec<DealWorkspaceParticipant>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceParticipantRow>(
                r#"
                select
                  dp.id::text as id,
                  dp.role as role_category,
                  dp.role_label,
                  dp.person_id::text as person_id,
                  dp.user_id::text as user_id,
                  person.display_name as person_name,
                  app_user.display_name as user_name,
                  person_email.identity_value as person_email,
                  person_phone.identity_value as person_phone
                from deal_participant dp
                left join person on person.id=dp.person_id
                left join app_user on app_user.id=dp.user_id
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=dp.person_id
                    and pi.identity_type='email'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) person_email on true
                left join lateral (
                  select pi.identity_value
                  from person_identity pi
                  where pi.person_id=dp.person_id
                    and pi.identity_type='phone'
                  order by pi.is_primary desc, pi.created_at asc
                  limit 1
                ) person_phone on true
                where dp.deal_id=$1::uuid
                  and dp.active=true
                order by
                  case dp.role
                    when 'client' then 0
                    when 'owner' then 1
                    when 'seller' then 2
                    else 3
                  end,
                  dp.created_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.participants", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let is_person = row.person_id.is_some();
                let detail = if is_person {
                    let mut values = Vec::new();
                    if let Some(email) = row.person_email.as_deref() {
                        if !email.trim().is_empty() {
                            values.push(email.to_owned());
                        }
                    }
                    if let Some(phone) = row.person_phone.as_deref() {
                        if !phone.trim().is_empty() {
                            values.push(phone.to_owned());
                        }
                    }
                    (!values.is_empty()).then(|| values.join(" · "))
                } else {
                    None
                };
                DealWorkspaceParticipant {
                    id: row.id,
                    role_category: row.role_category,
                    role_label: row.role_label,
                    kind: if is_person { "person" } else { "user" }.into(),
                    person_id: row.person_id,
                    user_id: row.user_id,
                    name: row
                        .person_name
                        .or(row.user_name)
                        .unwrap_or_else(|| "Unknown".into()),
                    detail,
                }
            })
            .collect())
    }

    async fn workspace_offers(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceOffer>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceOfferRow>(
                r#"
                select
                  o.id::text as id,
                  o.person_id::text as person_id,
                  person.display_name as person_name,
                  o.parent_offer_id::text as parent_offer_id,
                  o.amount::text as amount,
                  o.status,
                  to_char(
                    o.submitted_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as submitted_at_label,
                  case when o.responded_at is not null
                    then to_char(
                      o.responded_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as responded_at_label,
                  o.note
                from offer o
                left join person on person.id=o.person_id
                where o.deal_id=$1::uuid
                order by o.submitted_at asc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.offers", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceOffer {
                id: row.id,
                person_id: row.person_id,
                person_name: row.person_name,
                parent_offer_id: row.parent_offer_id.clone(),
                amount: row.amount.parse::<f64>().unwrap_or_default(),
                status: row.status,
                submitted_at_label: row.submitted_at_label,
                responded_at_label: row.responded_at_label,
                note: row.note,
                is_counter: row.parent_offer_id.is_some(),
            })
            .collect())
    }

    async fn workspace_showings(&self, deal_id: &str) -> DbResult<Vec<DealWorkspaceShowing>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, WorkspaceShowingRow>(
                r#"
                select
                  s.id::text as id,
                  s.person_id::text as person_id,
                  person.display_name as person_name,
                  s.status,
                  to_char(
                    s.requested_at at time zone 'America/Puerto_Rico',
                    'Mon FMDD, YYYY HH12:MI AM'
                  ) as requested_at_label,
                  case when s.scheduled_at is not null
                    then to_char(
                      s.scheduled_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as scheduled_at_label,
                  case when s.completed_at is not null
                    then to_char(
                      s.completed_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as completed_at_label,
                  case when s.cancelled_at is not null
                    then to_char(
                      s.cancelled_at at time zone 'America/Puerto_Rico',
                      'Mon FMDD, YYYY HH12:MI AM'
                    )
                  end as cancelled_at_label,
                  s.feedback
                from showing s
                join person on person.id=s.person_id
                where s.deal_id=$1::uuid
                order by
                  case s.status
                    when 'requested' then 0
                    when 'scheduled' then 1
                    when 'completed' then 2
                    when 'cancelled' then 3
                    else 4
                  end,
                  s.requested_at desc
                "#,
            )
            .bind(deal_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.showings", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealWorkspaceShowing {
                id: row.id,
                person_id: row.person_id,
                person_name: row.person_name,
                status: row.status,
                requested_at_label: row.requested_at_label,
                scheduled_at_label: row.scheduled_at_label,
                completed_at_label: row.completed_at_label,
                cancelled_at_label: row.cancelled_at_label,
                feedback: row.feedback,
            })
            .collect())
    }

    async fn contracts_for_property(
        &self,
        property_id: &str,
    ) -> DbResult<Vec<DealContractPortfolioItem>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, ContractRow>(
                r#"
                select
                  c.id::text as id,
                  c.form_template_id,
                  c.contract_type,
                  cp.property_id::text as property_id,
                  p.name as property_label,
                  c.status,
                  c.process_instance_id::text as process_instance_id,
                  c.executed_at::text as executed_at,
                  c.created_at::text as created_at
                from contract c
                join contract_property cp on cp.contract_id=c.id
                join role r on r.id=cp.role_id and r.scope=cp.role_scope
                left join property p on p.id=cp.property_id
                where r.scope='contract_property'
                  and r.code='SUBJECT_PROPERTY'
                  and cp.property_id=$1::uuid
                order by c.created_at desc,c.id
                "#,
            )
            .bind(property_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.workspace.contracts", &error))
        })?;
        Ok(rows
            .into_iter()
            .map(|row| DealContractPortfolioItem {
                id: row.id,
                form_template_id: row.form_template_id,
                contract_type: row.contract_type,
                property_id: row.property_id,
                property_label: row.property_label,
                status: row.status,
                process_instance_id: row.process_instance_id,
                executed_at: row.executed_at,
                created_at: row.created_at,
            })
            .collect())
    }

    async fn deals(&self) -> DbResult<Vec<DealPortfolioItem>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, DealRow>(
                r#"
                select
                  d.id::text as id,
                  p.id::text as property_id,
                  coalesce(p.name,'Property') as property_name,
                  p.location as property_location,
                  p.property_type,
                  p.bedrooms::text as bedrooms,
                  hero_media.media_id::text as hero_media_id,
                  client.id::text as client_id,
                  client.display_name as client_name,
                  d.stage,
                  d.list_price::text as list_price,
                  d.offer_price::text as offer_price,
                  owner.display_name as owner_name,
                  case when d.closing_date is not null
                       then to_char(d.closing_date,'Mon FMDD, YYYY') end as closing_date,
                  (select count(*) from showing s where s.deal_id=d.id)::bigint as showing_count,
                  (select count(*) from offer o where o.deal_id=d.id)::bigint as offer_count,
                  (select count(*) from deal_participant dp
                    where dp.deal_id=d.id and dp.active=true)::bigint as participant_count,
                  latest_offer.amount::text as latest_offer_amount,
                  latest_offer.status as latest_offer_status,
                  next_task.title as next_milestone,
                  case when next_task.due_at is not null
                       then to_char(next_task.due_at at time zone 'America/Puerto_Rico','Mon FMDD, YYYY')
                  end as next_milestone_at,
                  last_interaction.title as last_activity,
                  case when last_interaction.occurred_at is not null
                       then to_char(last_interaction.occurred_at at time zone 'America/Puerto_Rico','Mon FMDD, YYYY')
                  end as last_activity_at
                from deal d
                join property p on p.id=d.property_id
                join lateral (
                  select person.id, person.display_name
                  from deal_participant dp
                  join person on person.id=dp.person_id
                  where dp.deal_id=d.id and dp.role='client' and dp.active=true
                  order by dp.created_at asc
                  limit 1
                ) client on true
                left join lateral (
                  select app_user.display_name
                  from deal_participant dp
                  join app_user on app_user.id=dp.user_id
                  where dp.deal_id=d.id and dp.role='owner' and dp.active=true
                  order by dp.created_at asc
                  limit 1
                ) owner on true
                left join lateral (
                  select pm.media_id
                  from property_media pm
                  where pm.property_id=p.id and pm.role='hero'
                  order by pm.sort_order asc, pm.created_at asc
                  limit 1
                ) hero_media on true
                left join lateral (
                  select t.title,t.due_at
                  from task t
                  where t.deal_id=d.id and t.status='open'
                  order by t.due_at asc nulls last,t.created_at asc
                  limit 1
                ) next_task on true
                left join lateral (
                  select i.title,i.occurred_at
                  from interaction i
                  where i.deal_id=d.id
                  order by i.occurred_at desc
                  limit 1
                ) last_interaction on true
                left join lateral (
                  select o.amount,o.status
                  from offer o
                  where o.deal_id=d.id
                  order by o.submitted_at desc
                  limit 1
                ) latest_offer on true
                order by
                  case d.stage
                    when 'under_contract' then 1
                    when 'offer' then 2
                    when 'showing' then 3
                    when 'qualified' then 4
                    when 'new_lead' then 5
                    when 'closed' then 6
                    else 7
                  end,
                  d.updated_at desc
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.portfolio.list", &error))
        })?;

        Ok(rows.into_iter().map(map_deal).collect())
    }

    async fn contracts(&self) -> DbResult<Vec<DealContractPortfolioItem>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, ContractRow>(
                r#"
                select
                  c.id::text as id,
                  c.form_template_id,
                  c.contract_type,
                  cp.property_id::text as property_id,
                  p.name as property_label,
                  c.status,
                  c.process_instance_id::text as process_instance_id,
                  c.executed_at::text as executed_at,
                  c.created_at::text as created_at
                from contract c
                join contract_property cp on cp.contract_id=c.id
                join role r on r.id=cp.role_id and r.scope=cp.role_scope
                left join property p on p.id=cp.property_id
                where r.scope='contract_property'
                  and r.code='SUBJECT_PROPERTY'
                order by c.created_at desc,c.id
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.portfolio.contracts", &error))
        })?;

        Ok(rows.into_iter().map(|row| DealContractPortfolioItem {
            id: row.id,
            form_template_id: row.form_template_id,
            contract_type: row.contract_type,
            property_id: row.property_id,
            property_label: row.property_label,
            status: row.status,
            process_instance_id: row.process_instance_id,
            executed_at: row.executed_at,
            created_at: row.created_at,
        }).collect())
    }

    async fn dealable_properties(&self) -> DbResult<Vec<DealableProperty>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, PropertyRow>(
                r#"
                select id::text as id,name,location
                from property
                where archived_at is null
                order by name asc nulls last,id
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.portfolio.properties", &error))
        })?;
        Ok(rows.into_iter().map(|row| DealableProperty {
            id: row.id,
            name: row.name.unwrap_or_else(|| "Property".into()),
            location: row.location,
        }).collect())
    }

    async fn owner_candidates(&self) -> DbResult<Vec<DealOwnerCandidate>> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, UserRow>(
                r#"
                select id::text as id,display_name,email
                from app_user
                where active=true
                order by display_name asc,id
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("deal.portfolio.users", &error))
        })?;
        Ok(rows.into_iter().map(|row| DealOwnerCandidate {
            id: row.id,
            display_name: row.display_name,
            email: row.email,
        }).collect())
    }
}

fn parse_number(value: Option<&str>) -> Option<f64> {
    value.and_then(|value| value.parse::<f64>().ok())
}

fn map_deal(row: DealRow) -> DealPortfolioItem {
    let mut descriptor = Vec::new();
    if let Some(bedrooms) = row.bedrooms.as_deref().and_then(|value| value.parse::<f64>().ok()) {
        descriptor.push(format!(
            "{} bedrooms",
            if bedrooms.fract().abs() < 0.000_001 {
                format!("{bedrooms:.0}")
            } else {
                bedrooms.to_string()
            }
        ));
    }
    if let Some(property_type) = row.property_type.as_deref() {
        if !property_type.trim().is_empty() {
            descriptor.push(property_type.to_owned());
        }
    }

    DealPortfolioItem {
        id: row.id,
        property_id: row.property_id,
        property_name: row.property_name,
        property_location: row.property_location.unwrap_or_else(|| "Culebra, Puerto Rico".into()),
        property_descriptor: (!descriptor.is_empty()).then(|| descriptor.join(" · ")),
        hero_media_id: row.hero_media_id,
        client_id: row.client_id,
        client_name: row.client_name,
        stage: row.stage,
        list_price: parse_number(row.list_price.as_deref()),
        offer_price: parse_number(row.offer_price.as_deref()),
        owner: row.owner_name.unwrap_or_else(|| "Unassigned".into()),
        closing_date: row.closing_date,
        next_milestone: row.next_milestone,
        next_milestone_at: row.next_milestone_at,
        last_activity: row.last_activity,
        last_activity_at: row.last_activity_at,
        showing_count: row.showing_count,
        offer_count: row.offer_count,
        participant_count: row.participant_count,
        latest_offer_amount: parse_number(row.latest_offer_amount.as_deref()),
        latest_offer_status: row.latest_offer_status,
    }
}
