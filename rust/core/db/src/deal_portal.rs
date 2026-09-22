use domain::{
    CreateDealRequest, CreateDealResult, DealContractPortfolioItem, DealOwnerCandidate,
    DealPortfolioItem, DealPortfolioSnapshot, DealableProperty,
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
