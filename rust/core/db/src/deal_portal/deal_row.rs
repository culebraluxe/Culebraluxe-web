//! Moved from `deal_portal.rs` (move only): DealRow, ContractRow, PropertyRow, UserRow, WorkspaceHeaderRow, WorkspaceTaskRow, WorkspaceActivityRow, WorkspaceParticipantRow, WorkspaceOfferRow, WorkspaceShowingRow, DealPortalDao, new.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, FromRow)]
pub(super) struct DealRow {
    pub(super) id: String,
    pub(super) property_id: String,
    pub(super) property_name: String,
    pub(super) property_location: Option<String>,
    pub(super) property_type: Option<String>,
    pub(super) bedrooms: Option<String>,
    pub(super) hero_media_id: Option<String>,
    pub(super) client_id: String,
    pub(super) client_name: String,
    pub(super) stage: String,
    pub(super) list_price: Option<String>,
    pub(super) offer_price: Option<String>,
    pub(super) owner_name: Option<String>,
    pub(super) closing_date: Option<String>,
    pub(super) showing_count: i64,
    pub(super) offer_count: i64,
    pub(super) participant_count: i64,
    pub(super) latest_offer_amount: Option<String>,
    pub(super) latest_offer_status: Option<String>,
    pub(super) next_milestone: Option<String>,
    pub(super) next_milestone_at: Option<String>,
    pub(super) last_activity: Option<String>,
    pub(super) last_activity_at: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct ContractRow {
    pub(super) id: String,
    pub(super) form_template_id: String,
    pub(super) contract_type: String,
    pub(super) property_id: String,
    pub(super) property_label: Option<String>,
    pub(super) status: String,
    pub(super) process_instance_id: Option<String>,
    pub(super) executed_at: Option<String>,
    pub(super) created_at: String,
}

#[derive(Debug, FromRow)]
pub(super) struct PropertyRow {
    pub(super) id: String,
    pub(super) name: Option<String>,
    pub(super) location: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct UserRow {
    pub(super) id: String,
    pub(super) display_name: String,
    pub(super) email: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct WorkspaceHeaderRow {
    pub(super) deal_id: String,
    pub(super) stage: String,
    pub(super) list_price: Option<String>,
    pub(super) offer_price: Option<String>,
    pub(super) closing_date_label: Option<String>,
    pub(super) closed_at_label: Option<String>,
    pub(super) notes: Option<String>,
    pub(super) created_at_label: String,
    pub(super) updated_at_label: String,
    pub(super) property_id: String,
    pub(super) property_name: String,
    pub(super) property_location: Option<String>,
    pub(super) property_type: Option<String>,
    pub(super) bedrooms: Option<String>,
    pub(super) bathrooms: Option<String>,
    pub(super) square_feet: Option<i64>,
    pub(super) client_id: String,
    pub(super) client_name: String,
    pub(super) client_role: String,
    pub(super) client_status: String,
    pub(super) client_email: Option<String>,
    pub(super) client_phone: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct WorkspaceTaskRow {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) detail: Option<String>,
    pub(super) due_at_label: Option<String>,
    pub(super) is_overdue: bool,
}

#[derive(Debug, FromRow)]
pub(super) struct WorkspaceActivityRow {
    pub(super) id: String,
    pub(super) person_id: Option<String>,
    pub(super) channel: String,
    pub(super) direction: Option<String>,
    pub(super) occurred_at_label: String,
    pub(super) title: Option<String>,
    pub(super) summary: Option<String>,
    pub(super) person_name: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct WorkspaceParticipantRow {
    pub(super) id: String,
    pub(super) role_category: String,
    pub(super) role_label: Option<String>,
    pub(super) person_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) person_name: Option<String>,
    pub(super) user_name: Option<String>,
    pub(super) person_email: Option<String>,
    pub(super) person_phone: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct WorkspaceOfferRow {
    pub(super) id: String,
    pub(super) person_id: String,
    pub(super) person_name: Option<String>,
    pub(super) parent_offer_id: Option<String>,
    pub(super) amount: String,
    pub(super) financing_type: Option<String>,
    pub(super) deposit_amount: Option<String>,
    pub(super) inspection_days: Option<i32>,
    pub(super) seller_credits: Option<String>,
    pub(super) proposed_closing_date: Option<String>,
    pub(super) contingencies: Option<String>,
    pub(super) expires_at_label: Option<String>,
    pub(super) status: String,
    pub(super) submitted_at_label: String,
    pub(super) responded_at_label: Option<String>,
    pub(super) note: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct WorkspaceShowingRow {
    pub(super) id: String,
    pub(super) person_id: String,
    pub(super) person_name: String,
    pub(super) status: String,
    pub(super) requested_at_label: String,
    pub(super) scheduled_at_label: Option<String>,
    pub(super) completed_at_label: Option<String>,
    pub(super) cancelled_at_label: Option<String>,
    pub(super) feedback: Option<String>,
}

#[derive(Clone)]
pub struct DealPortalDao {
    pub(super) db: Database,
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
            .bind(
                request
                    .notes
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
            )
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
}
