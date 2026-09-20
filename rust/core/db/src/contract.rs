use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    Contract, ContractEffectiveState, ContractFacts, ContractRole, ContractSummary,
    CreateContractFromFormRequest, ExecuteContractRequest, SaveContractDraftRequest,
};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct ContractRow {
    id: String,
    contract_type: String,
    form_template_id: String,
    source_form_instance_id: Option<String>,
    predecessor_contract_id: Option<String>,
    process_instance_id: Option<String>,
    facts: Value,
    status: String,
    executed_at: Option<DateTime<Utc>>,
    evidence_document_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct PersonRoleRow {
    person_id: String,
    role_code: String,
    ordinal: i32,
    snapshot_name: Option<String>,
    attributes: Value,
}

#[derive(Debug, FromRow)]
struct FirmRoleRow {
    firm_id: String,
    role_code: String,
    ordinal: i32,
    snapshot_name: Option<String>,
    attributes: Value,
}

#[derive(Debug, FromRow)]
struct SummaryRow {
    id: String,
    contract_type: String,
    form_template_id: String,
    status: String,
    property_id: String,
    predecessor_contract_id: Option<String>,
    process_instance_id: Option<String>,
    evidence_document_id: Option<String>,
    executed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ChainRow {
    id: String,
    facts: Value,
    depth: i32,
}

fn facts(value: Value) -> ContractFacts {
    match value {
        Value::Object(map) => map,
        _ => ContractFacts::new(),
    }
}

async fn load_contract_pool(db: &Database, contract_id: &str) -> DbResult<Option<Contract>> {
    let row = sqlx::query_as::<_, ContractRow>(
        r#"
        select id::text as id, contract_type, form_template_id,
               source_form_instance_id::text as source_form_instance_id,
               predecessor_contract_id::text as predecessor_contract_id,
               process_instance_id::text as process_instance_id,
               facts, status, executed_at,
               evidence_document_id::text as evidence_document_id
        from contract
        where id = $1::uuid
        limit 1
        "#,
    )
    .bind(contract_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.get", &error))?;

    let Some(row) = row else {
        return Ok(None);
    };

    let property_id = sqlx::query_scalar::<_, String>(
        r#"
        select cp.property_id::text
        from contract_property cp
        join role r on r.id = cp.role_id and r.scope = cp.role_scope
        where cp.contract_id = $1::uuid
          and r.scope = 'contract_property'
          and r.code = 'SUBJECT_PROPERTY'
        order by cp.ordinal, cp.id
        limit 1
        "#,
    )
    .bind(contract_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.get.property", &error))?
    .ok_or_else(|| {
        DbFailure::schema_mismatch(
            "contract.get.property",
            format!("Contract {contract_id} has no SUBJECT_PROPERTY mapping"),
        )
    })?;

    let person_roles = sqlx::query_as::<_, PersonRoleRow>(
        r#"
        select cp.person_id::text as person_id, r.code as role_code,
               cp.ordinal, cp.snapshot_name, cp.attributes
        from contract_person cp
        join role r on r.id = cp.role_id and r.scope = cp.role_scope
        where cp.contract_id = $1::uuid
        order by r.code, cp.ordinal, cp.id
        "#,
    )
    .bind(contract_id)
    .fetch_all(db.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.get.person_roles", &error))?;

    let firm_roles = sqlx::query_as::<_, FirmRoleRow>(
        r#"
        select cf.firm_id::text as firm_id, r.code as role_code,
               cf.ordinal, cf.snapshot_name, cf.attributes
        from contract_firm cf
        join role r on r.id = cf.role_id and r.scope = cf.role_scope
        where cf.contract_id = $1::uuid
        order by r.code, cf.ordinal, cf.id
        "#,
    )
    .bind(contract_id)
    .fetch_all(db.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.get.firm_roles", &error))?;

    Ok(Some(map_contract(row, property_id, person_roles, firm_roles)))
}

async fn load_contract_tx(
    tx: &mut DbTransaction,
    contract_id: &str,
) -> DbResult<Option<Contract>> {
    let row = sqlx::query_as::<_, ContractRow>(
        r#"
        select id::text as id, contract_type, form_template_id,
               source_form_instance_id::text as source_form_instance_id,
               predecessor_contract_id::text as predecessor_contract_id,
               process_instance_id::text as process_instance_id,
               facts, status, executed_at,
               evidence_document_id::text as evidence_document_id
        from contract
        where id = $1::uuid
        limit 1
        "#,
    )
    .bind(contract_id)
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.tx.get", &error))?;

    let Some(row) = row else {
        return Ok(None);
    };

    let property_id = sqlx::query_scalar::<_, String>(
        r#"
        select cp.property_id::text
        from contract_property cp
        join role r on r.id = cp.role_id and r.scope = cp.role_scope
        where cp.contract_id = $1::uuid
          and r.scope = 'contract_property'
          and r.code = 'SUBJECT_PROPERTY'
        order by cp.ordinal, cp.id
        limit 1
        "#,
    )
    .bind(contract_id)
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.tx.get.property", &error))?
    .ok_or_else(|| {
        DbFailure::schema_mismatch(
            "contract.tx.get.property",
            format!("Contract {contract_id} has no SUBJECT_PROPERTY mapping"),
        )
    })?;

    let person_roles = sqlx::query_as::<_, PersonRoleRow>(
        r#"
        select cp.person_id::text as person_id, r.code as role_code,
               cp.ordinal, cp.snapshot_name, cp.attributes
        from contract_person cp
        join role r on r.id = cp.role_id and r.scope = cp.role_scope
        where cp.contract_id = $1::uuid
        order by r.code, cp.ordinal, cp.id
        "#,
    )
    .bind(contract_id)
    .fetch_all(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.tx.get.person_roles", &error))?;

    let firm_roles = sqlx::query_as::<_, FirmRoleRow>(
        r#"
        select cf.firm_id::text as firm_id, r.code as role_code,
               cf.ordinal, cf.snapshot_name, cf.attributes
        from contract_firm cf
        join role r on r.id = cf.role_id and r.scope = cf.role_scope
        where cf.contract_id = $1::uuid
        order by r.code, cf.ordinal, cf.id
        "#,
    )
    .bind(contract_id)
    .fetch_all(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.tx.get.firm_roles", &error))?;

    Ok(Some(map_contract(row, property_id, person_roles, firm_roles)))
}

fn map_contract(
    row: ContractRow,
    property_id: String,
    person_roles: Vec<PersonRoleRow>,
    firm_roles: Vec<FirmRoleRow>,
) -> Contract {
    let mut roles = Vec::with_capacity(person_roles.len() + firm_roles.len());
    roles.extend(person_roles.into_iter().map(|role| ContractRole::Person {
        person_id: role.person_id,
        role_code: role.role_code,
        ordinal: role.ordinal,
        snapshot_name: role.snapshot_name,
        attributes: facts(role.attributes),
    }));
    roles.extend(firm_roles.into_iter().map(|role| ContractRole::Firm {
        firm_id: role.firm_id,
        role_code: role.role_code,
        ordinal: role.ordinal,
        snapshot_name: role.snapshot_name,
        attributes: facts(role.attributes),
    }));

    Contract {
        id: row.id,
        contract_type: row.contract_type,
        form_template_id: row.form_template_id,
        source_form_instance_id: row.source_form_instance_id,
        predecessor_contract_id: row.predecessor_contract_id,
        process_instance_id: row.process_instance_id,
        property_id,
        roles,
        facts: facts(row.facts),
        status: row.status,
        executed_at: row.executed_at.map(|value| value.to_rfc3339()),
        evidence_document_id: row.evidence_document_id,
    }
}

async fn role_id(
    tx: &mut DbTransaction,
    scope: &str,
    code: &str,
) -> DbResult<String> {
    sqlx::query_scalar::<_, String>(
        r#"
        select id::text
        from role
        where scope = $1 and code = $2 and active = true
        limit 1
        "#,
    )
    .bind(scope)
    .bind(code.trim().to_uppercase())
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.role_id", &error))?
    .ok_or_else(|| {
        DbFailure::schema_mismatch(
            "contract.role_id",
            format!("unknown active Role {scope}:{code}"),
        )
    })
}

async fn replace_mappings(
    tx: &mut DbTransaction,
    request: &SaveContractDraftRequest,
) -> DbResult<()> {
    for table in ["contract_property", "contract_person", "contract_firm"] {
        let statement = format!("delete from {table} where contract_id = $1::uuid");
        sqlx::query(&statement)
            .bind(&request.contract_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("contract.replace_mappings.delete", &error))?;
    }

    let subject_role_id = role_id(tx, "contract_property", "SUBJECT_PROPERTY").await?;
    sqlx::query(
        r#"
        insert into contract_property (
            contract_id, property_id, role_id, role_scope, ordinal
        )
        values ($1::uuid, $2::uuid, $3::uuid, 'contract_property', 0)
        "#,
    )
    .bind(&request.contract_id)
    .bind(&request.property_id)
    .bind(subject_role_id)
    .execute(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("contract.replace_mappings.property", &error))?;

    for role in &request.roles {
        match role {
            ContractRole::Person {
                person_id,
                role_code,
                ordinal,
                snapshot_name,
                attributes,
            } => {
                let id = role_id(tx, "contract_person", role_code).await?;
                sqlx::query(
                    r#"
                    insert into contract_person (
                        contract_id, person_id, role_id, role_scope,
                        ordinal, snapshot_name, attributes
                    )
                    values ($1::uuid,$2::uuid,$3::uuid,'contract_person',$4,$5,$6)
                    "#,
                )
                .bind(&request.contract_id)
                .bind(person_id)
                .bind(id)
                .bind(*ordinal)
                .bind(snapshot_name.as_deref())
                .bind(Value::Object(attributes.clone()))
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("contract.replace_mappings.person", &error))?;
            }
            ContractRole::Firm {
                firm_id,
                role_code,
                ordinal,
                snapshot_name,
                attributes,
            } => {
                let id = role_id(tx, "contract_firm", role_code).await?;
                sqlx::query(
                    r#"
                    insert into contract_firm (
                        contract_id, firm_id, role_id, role_scope,
                        ordinal, snapshot_name, attributes
                    )
                    values ($1::uuid,$2::uuid,$3::uuid,'contract_firm',$4,$5,$6)
                    "#,
                )
                .bind(&request.contract_id)
                .bind(firm_id)
                .bind(id)
                .bind(*ordinal)
                .bind(snapshot_name.as_deref())
                .bind(Value::Object(attributes.clone()))
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("contract.replace_mappings.firm", &error))?;
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct ContractDao {
    db: Database,
}

impl ContractDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, contract_id: &str) -> DbResult<Option<Contract>> {
        load_contract_pool(&self.db, contract_id).await
    }

    pub async fn list(&self) -> DbResult<Vec<ContractSummary>> {
        self.list_for_process_instance_inner(None).await
    }

    pub async fn list_for_process_instance(
        &self,
        process_instance_id: &str,
    ) -> DbResult<Vec<ContractSummary>> {
        self.list_for_process_instance_inner(Some(process_instance_id))
            .await
    }

    async fn list_for_process_instance_inner(
        &self,
        process_instance_id: Option<&str>,
    ) -> DbResult<Vec<ContractSummary>> {
        let rows = sqlx::query_as::<_, SummaryRow>(
            r#"
            select c.id::text as id, c.contract_type, c.form_template_id, c.status,
                   cp.property_id::text as property_id,
                   c.predecessor_contract_id::text as predecessor_contract_id,
                   c.process_instance_id::text as process_instance_id,
                   c.evidence_document_id::text as evidence_document_id,
                   c.executed_at, c.created_at
            from contract c
            join contract_property cp on cp.contract_id = c.id
            join role r on r.id = cp.role_id and r.scope = cp.role_scope
            where r.scope = 'contract_property'
              and r.code = 'SUBJECT_PROPERTY'
              and ($1::uuid is null or c.process_instance_id = $1::uuid)
            order by c.created_at desc, c.id
            "#,
        )
        .bind(process_instance_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("contract.list", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| ContractSummary {
                id: row.id,
                contract_type: row.contract_type,
                form_template_id: row.form_template_id,
                status: row.status,
                property_id: row.property_id,
                predecessor_contract_id: row.predecessor_contract_id,
                process_instance_id: row.process_instance_id,
                evidence_document_id: row.evidence_document_id,
                executed_at: row.executed_at.map(|value| value.to_rfc3339()),
                created_at: row.created_at.to_rfc3339(),
            })
            .collect())
    }

    pub async fn create_from_form(
        &self,
        request: &CreateContractFromFormRequest,
    ) -> DbResult<Contract> {
        let mut tx = self.db.begin("contract.create_from_form").await?;
        let result = async {
            let exists = sqlx::query_scalar::<_, bool>(
                "select exists(select 1 from contract where id = $1::uuid)",
            )
            .bind(&request.contract_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("contract.create.exists", &error))?;
            if exists {
                return Err(DbFailure::schema_mismatch(
                    "contract.create",
                    format!("Contract already exists: {}", request.contract_id),
                ));
            }

            sqlx::query(
                r#"
                insert into contract (
                    id, contract_type, form_template_id, source_form_instance_id,
                    predecessor_contract_id, process_instance_id, facts, status
                )
                values ($1::uuid,$2,$3,$4::uuid,$5::uuid,$6::uuid,$7,'draft')
                "#,
            )
            .bind(&request.contract_id)
            .bind(request.contract_type.trim())
            .bind(request.form_template_id.trim())
            .bind(request.source_form_instance_id.as_deref())
            .bind(request.predecessor_contract_id.as_deref())
            .bind(request.process_instance_id.as_deref())
            .bind(Value::Object(request.facts.clone()))
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("contract.create.insert", &error))?;

            replace_mappings(&mut tx, request).await?;
            load_contract_tx(&mut tx, &request.contract_id)
                .await?
                .ok_or_else(|| {
                    DbFailure::schema_mismatch(
                        "contract.create",
                        "contract disappeared after creation",
                    )
                })
        }
        .await;

        match result {
            Ok(contract) => {
                tx.commit().await?;
                Ok(contract)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn save_draft(&self, request: &SaveContractDraftRequest) -> DbResult<Contract> {
        let mut tx = self.db.begin("contract.save_draft").await?;
        let result = async {
            let status = sqlx::query_scalar::<_, String>(
                "select status from contract where id=$1::uuid for update",
            )
            .bind(&request.contract_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("contract.save_draft.lock", &error))?;

            if let Some(status) = status.as_deref() {
                if status != "draft" {
                    return Err(DbFailure::schema_mismatch(
                        "contract.save_draft",
                        format!(
                            "Contract {} is {status}; only draft Contracts may be replaced",
                            request.contract_id
                        ),
                    ));
                }
                sqlx::query(
                    r#"
                    update contract
                    set contract_type=$2, form_template_id=$3,
                        source_form_instance_id=$4::uuid,
                        predecessor_contract_id=$5::uuid,
                        process_instance_id=$6::uuid,
                        facts=$7, updated_at=now()
                    where id=$1::uuid
                    "#,
                )
                .bind(&request.contract_id)
                .bind(request.contract_type.trim())
                .bind(request.form_template_id.trim())
                .bind(request.source_form_instance_id.as_deref())
                .bind(request.predecessor_contract_id.as_deref())
                .bind(request.process_instance_id.as_deref())
                .bind(Value::Object(request.facts.clone()))
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("contract.save_draft.update", &error))?;
            } else {
                sqlx::query(
                    r#"
                    insert into contract (
                        id, contract_type, form_template_id, source_form_instance_id,
                        predecessor_contract_id, process_instance_id, facts, status
                    )
                    values ($1::uuid,$2,$3,$4::uuid,$5::uuid,$6::uuid,$7,'draft')
                    "#,
                )
                .bind(&request.contract_id)
                .bind(request.contract_type.trim())
                .bind(request.form_template_id.trim())
                .bind(request.source_form_instance_id.as_deref())
                .bind(request.predecessor_contract_id.as_deref())
                .bind(request.process_instance_id.as_deref())
                .bind(Value::Object(request.facts.clone()))
                .execute(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("contract.save_draft.insert", &error))?;
            }

            replace_mappings(&mut tx, request).await?;
            load_contract_tx(&mut tx, &request.contract_id)
                .await?
                .ok_or_else(|| {
                    DbFailure::schema_mismatch(
                        "contract.save_draft",
                        "contract disappeared after draft save",
                    )
                })
        }
        .await;

        match result {
            Ok(contract) => {
                tx.commit().await?;
                Ok(contract)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn get_effective_state(
        &self,
        contract_id: &str,
    ) -> DbResult<Option<ContractEffectiveState>> {
        let rows = sqlx::query_as::<_, ChainRow>(
            r#"
            with recursive chain as (
              select c.id, c.predecessor_contract_id, c.facts,
                     0::int as depth, array[c.id]::uuid[] as path
              from contract c where c.id = $1::uuid
              union all
              select parent.id, parent.predecessor_contract_id, parent.facts,
                     child.depth + 1, child.path || parent.id
              from contract parent
              join chain child on child.predecessor_contract_id = parent.id
              where child.depth < 100 and not parent.id = any(child.path)
            )
            select id::text as id, facts, depth
            from chain
            order by depth desc
            "#,
        )
        .bind(contract_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("contract.get_effective_state", &error))?;

        if rows.is_empty() {
            return Ok(None);
        }

        let mut effective = ContractFacts::new();
        let mut source_contract_ids = Vec::with_capacity(rows.len());
        for row in rows {
            let _ = row.depth;
            effective.extend(facts(row.facts));
            source_contract_ids.push(row.id);
        }
        Ok(Some(ContractEffectiveState {
            contract_id: contract_id.to_owned(),
            facts: effective,
            source_contract_ids,
        }))
    }

    pub async fn execute(&self, request: &ExecuteContractRequest) -> DbResult<Option<Contract>> {
        let updated = sqlx::query_scalar::<_, String>(
            r#"
            update contract
            set status='executed',
                executed_at=coalesce(executed_at, now()),
                evidence_document_id=coalesce($2::uuid, evidence_document_id),
                updated_at=now()
            where id=$1::uuid
            returning id::text
            "#,
        )
        .bind(&request.contract_id)
        .bind(request.evidence_document_id.as_deref())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("contract.execute", &error))?;

        if updated.is_none() {
            return Ok(None);
        }
        self.get(&request.contract_id).await
    }
}
