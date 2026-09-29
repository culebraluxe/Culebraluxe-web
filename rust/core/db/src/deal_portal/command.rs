//! Moved from `deal_portal.rs` (move only): command.

#[allow(unused_imports)]
use super::*;

impl DealPortalDao {
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
                    financing_type,
                    deposit_amount,
                    inspection_days,
                    seller_credits,
                    proposed_closing_date,
                    contingencies,
                    expires_at,
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
                          deal_id, person_id, parent_offer_id, amount, status,
                          financing_type, deposit_amount, inspection_days, seller_credits,
                          proposed_closing_date, contingencies, expires_at
                        )
                        values (
                          $1::uuid,$2::uuid,$3::uuid,$4,'submitted',
                          nullif(trim($5),''),
                          nullif(trim($6),'')::numeric,
                          nullif(trim($7),'')::integer,
                          nullif(trim($8),'')::numeric,
                          nullif(trim($9),'')::date,
                          nullif(trim($10),''),
                          nullif(trim($11),'')::timestamptz
                        )
                        returning id::text
                        "#,
                    )
                    .bind(deal_id)
                    .bind(person_id)
                    .bind(parent_offer_id.as_deref())
                    .bind(amount)
                    .bind(financing_type.as_deref().unwrap_or(""))
                    .bind(deposit_amount.as_deref().unwrap_or(""))
                    .bind(inspection_days.as_deref().unwrap_or(""))
                    .bind(seller_credits.as_deref().unwrap_or(""))
                    .bind(proposed_closing_date.as_deref().unwrap_or(""))
                    .bind(contingencies.as_deref().unwrap_or(""))
                    .bind(expires_at.as_deref().unwrap_or(""))
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
                            and lower(role_label)=lower($2)
                        )
                        "#,
                    )
                    .bind(deal_id)
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

}
